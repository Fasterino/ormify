use std::{collections::HashMap, fmt::Debug, future::Future, pin::Pin};

use crate::{
    Condition, EntityId, Entry, Error, Execution, ExecutionResponse, Filters, Join, LimitLike,
    OrderBy, Query, QueryResponse, Scheme, Value,
};

/// Registered entity schemes known to an [`Ormify`] instance.
///
/// Schemes are registered during initialization and are used by the language
/// compiler to resolve table metadata and foreign-key dependencies.
#[derive(Clone, Default)]
pub struct SchemeRegistry {
    schemes: HashMap<EntityId, Scheme>,
}

impl SchemeRegistry {
    pub(crate) fn register(&mut self, entity_id: EntityId, scheme: Scheme) -> crate::Result<()> {
        if self.schemes.contains_key(&entity_id) {
            return Err(Error::make_invalid_schema_error(format!(
                "entity {entity_id:?} is already registered"
            )));
        }
        self.schemes.insert(entity_id, scheme);
        Ok(())
    }

    pub fn get(&self, entity_id: EntityId) -> crate::Result<&Scheme> {
        self.schemes
            .get(&entity_id)
            .ok_or_else(|| Error::EntityNotInitialized(entity_id))
    }
}

/// Strategy used to load relationship joins.
///
/// `Recursive` is the currently supported strategy and performs nested joins
/// through additional driver queries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JoinStrategy {
    Recursive,
}

/// Coordinates schema registration, SQL compilation, and database I/O.
///
/// `L` is the backend language and `D` is its driver. The application chooses
/// and runs the async executor; `Ormify` itself does not create a runtime.
pub struct Ormify<L: Language, D: Supported<L>> {
    language: L,
    driver: D,
    schemes: SchemeRegistry,
    join_strategy: JoinStrategy,
    #[cfg(feature = "bench")]
    bench_recorder: crate::BenchmarkRecorder,
}

impl<L: Language, D: Supported<L>> Ormify<L, D> {
    /// Connect a backend driver and initialize the engine's empty scheme registry.
    pub async fn connect<I: Into<D::ConnectArgs>>(args: I) -> crate::Result<Self> {
        let language = L::default();
        let driver = D::connect(args.into(), D::get_language()).await?;
        let schemes = SchemeRegistry::default();
        Ok(Self {
            language,
            driver,
            schemes,
            join_strategy: JoinStrategy::Recursive,
            #[cfg(feature = "bench")]
            bench_recorder: crate::BenchmarkRecorder::new(),
        })
    }

    /// Select the join loading strategy for subsequent queries.
    pub fn set_join_strategy(&mut self, strategy: JoinStrategy) {
        self.join_strategy = strategy;
    }

    /// Execute a select query and return rows with nested join responses.
    pub async fn query<I: Into<Query>>(&mut self, query: I) -> crate::Result<QueryResponse> {
        #[cfg(feature = "bench")]
        let mut benchmark_tool = self.bench_recorder.tool();

        let Query { entity_id, filters } = query.into();
        let Filters {
            condition,
            limit,
            offset,
            order_by,
            joins,
        } = filters;

        let schemed_query = SchemedQuery {
            scheme: self.get_scheme(entity_id)?,
            condition,
            limit: &limit,
            offset: &offset,
            order_by: &order_by,
        };

        let language_query = self.language.parse_single_query(schemed_query)?;

        let res = self
            .handle_query_with_joins(
                language_query,
                &joins,
                #[cfg(feature = "bench")]
                &mut benchmark_tool,
            )
            .await;

        #[cfg(feature = "bench")]
        benchmark_tool.finish(&mut self.bench_recorder);

        res
    }

    pub async fn execute(
        &mut self,
        transaction: Vec<Execution>,
    ) -> crate::Result<ExecutionResponse> {
        #[cfg(feature = "bench")]
        let mut benchmark_tool = self.bench_recorder.tool();

        let previous_schemes = self.schemes.clone();
        let init_order = match self.resolve_init_order(&transaction) {
            Ok(order) => order,
            Err(error) => {
                self.schemes = previous_schemes;
                return Err(error);
            }
        };
        let mut commands = Vec::with_capacity(transaction.len());

        for index in init_order {
            let Execution::Init { entity_id, .. } = &transaction[index] else {
                continue;
            };
            let Execution::Init { scheme, .. } = &transaction[index] else {
                unreachable!();
            };
            if let Err(error) = self.schemes.register(*entity_id, scheme.clone()) {
                self.schemes = previous_schemes.clone();
                return Err(error);
            }
            let mut subcommands = match self.handle_execution(transaction[index].clone()) {
                Ok(commands) => commands,
                Err(error) => {
                    self.schemes = previous_schemes;
                    return Err(error);
                }
            };
            commands.append(&mut subcommands);
        }

        for execution in transaction {
            if matches!(execution, Execution::Init { .. }) {
                continue;
            }
            let mut subcommands = match self.handle_execution(execution) {
                Ok(x) => x,
                Err(error) => {
                    self.schemes = previous_schemes;
                    return Err(error);
                }
            };
            commands.append(&mut subcommands);
        }

        #[cfg(feature = "bench")]
        benchmark_tool.driver_start();

        let res = self.driver.execute(commands).await;

        #[cfg(feature = "bench")]
        benchmark_tool.driver_end();

        #[cfg(feature = "bench")]
        benchmark_tool.finish(&mut self.bench_recorder);

        match res {
            Ok(response) => Ok(response),
            Err(error) => {
                self.schemes = previous_schemes;
                Err(error)
            }
        }
    }

    #[cfg(feature = "bench")]
    pub fn get_benchmark(self) -> crate::Benchmark {
        self.bench_recorder.finish()
    }

    fn resolve_init_order(&self, transaction: &[Execution]) -> crate::Result<Vec<usize>> {
        fn visit(
            index: usize,
            schemes: &HashMap<EntityId, &Scheme>,
            indexes: &HashMap<EntityId, usize>,
            state: &mut HashMap<EntityId, u8>,
            order: &mut Vec<usize>,
        ) -> crate::Result<()> {
            let entity_id = *indexes
                .iter()
                .find_map(|(entity_id, candidate)| (*candidate == index).then_some(entity_id))
                .ok_or_else(|| Error::make_invalid_schema_error("missing init entity"))?;
            match state.get(&entity_id).copied() {
                Some(1) => {
                    return Err(Error::make_invalid_schema_error(format!(
                        "cyclic entity dependency at {entity_id:?}"
                    )));
                }
                Some(2) => return Ok(()),
                _ => {}
            }
            state.insert(entity_id, 1);
            let scheme = schemes[&entity_id];
            for constraint in &scheme.constraints {
                if let crate::ConstraintType::ForeignKey {
                    entity_id: dependency,
                    ..
                } = constraint.inner
                {
                    if let Some(dependency_index) = indexes.get(&dependency) {
                        visit(*dependency_index, schemes, indexes, state, order)?;
                    }
                }
            }
            state.insert(entity_id, 2);
            order.push(index);
            Ok(())
        }

        let mut schemes = HashMap::new();
        let mut indexes = HashMap::new();
        for (index, execution) in transaction.iter().enumerate() {
            if let Execution::Init { entity_id, scheme } = execution {
                schemes.insert(*entity_id, scheme);
                indexes.insert(*entity_id, index);
            }
        }
        let mut state = HashMap::new();
        let mut order = Vec::with_capacity(indexes.len());
        for index in indexes.values().copied() {
            visit(index, &schemes, &indexes, &mut state, &mut order)?;
        }
        Ok(order)
    }

    fn get_scheme(&self, entity_id: EntityId) -> crate::Result<&Scheme> {
        self.schemes.get(entity_id)
    }

    fn handle_query_with_joins<'a>(
        &'a mut self,
        language_query: LanguageQuery,
        joins: &'a [Join],
        #[cfg(feature = "bench")] benchmark_tool: &'a mut crate::BenchmarkTool,
    ) -> Pin<Box<dyn Future<Output = crate::Result<QueryResponse>> + 'a>> {
        Box::pin(async move {
            #[cfg(feature = "bench")]
            benchmark_tool.driver_start();

            let entries = self.driver.query(language_query).await?;

            #[cfg(feature = "bench")]
            benchmark_tool.driver_end();

            match self.join_strategy {
                JoinStrategy::Recursive => {
                    self.handle_recursive_joins(
                        entries,
                        joins,
                        #[cfg(feature = "bench")]
                        benchmark_tool,
                    )
                    .await
                }
            }
        })
    }

    fn handle_recursive_joins<'a>(
        &'a mut self,
        entries: Vec<Entry>,
        joins: &'a [Join],
        #[cfg(feature = "bench")] benchmark_tool: &'a mut crate::BenchmarkTool,
    ) -> Pin<Box<dyn Future<Output = crate::Result<QueryResponse>> + 'a>> {
        Box::pin(async move {
            let mut joins_entries = vec![Vec::new(); entries.len()];
            for join in joins {
                let join_entries = self
                    .handle_query_join(
                        &entries,
                        join,
                        #[cfg(feature = "bench")]
                        benchmark_tool,
                    )
                    .await?;
                if join_entries.len() != entries.len() {
                    return Err(Error::make_invalid_join_error(format!(
                        "driver returned {} join result groups for {} parent rows",
                        join_entries.len(),
                        entries.len()
                    )));
                }
                for (i, join_entry) in join_entries.into_iter().enumerate() {
                    joins_entries[i].push(join_entry);
                }
            }

            Ok(QueryResponse(
                entries.into_iter().zip(joins_entries).collect(),
            ))
        })
    }

    fn handle_query_join<'a>(
        &'a mut self,
        entries: &'a [Entry],
        join: &'a Join,
        #[cfg(feature = "bench")] benchmark_tool: &'a mut crate::BenchmarkTool,
    ) -> Pin<Box<dyn Future<Output = crate::Result<Vec<QueryResponse>>> + 'a>> {
        Box::pin(async move {
            let Join {
                entity_id,
                filters,
                key_pairs,
            } = join;
            let Filters {
                condition: cond_opt,
                limit,
                offset,
                order_by,
                joins,
            } = filters;

            let mut iter = key_pairs.iter().map(|x| x.0.clone()).rev();

            let mut condition = Condition::Eq(
                iter.next().ok_or_else(|| {
                    Error::make_invalid_join_error("join must define at least one key pair")
                })?,
                Value::I32(0),
            );

            for name in iter {
                condition = Condition::Eq(name, Value::I32(0)) & condition;
            }

            if let Some(cond) = cond_opt {
                condition = condition & cond.clone();
            }

            let query_orig = self.language.parse_single_query(SchemedQuery {
                scheme: self.get_scheme(*entity_id)?,
                condition: Some(condition),
                limit,
                offset,
                order_by,
            })?;

            let mut results = Vec::with_capacity(entries.len());

            for entry in entries {
                let mut query = query_orig.clone();
                for i in 0..key_pairs.len() {
                    let name = &key_pairs[i].1;
                    let value = entry.get(name).ok_or_else(|| Error::TypeMismatch)?.clone();
                    let argument = query.args.get_mut(i).ok_or_else(|| {
                        Error::Database("language query parameter layout is inconsistent".into())
                    })?;
                    *argument = value;
                }
                let result = self
                    .handle_query_with_joins(
                        query,
                        &joins,
                        #[cfg(feature = "bench")]
                        benchmark_tool,
                    )
                    .await?;
                results.push(result);
            }

            Ok(results)
        })
    }

    fn handle_execution(&mut self, execution: Execution) -> crate::Result<Vec<LanguageQuery>> {
        let execution = match execution {
            Execution::Init {
                entity_id,
                scheme: _,
            } => SchemedExecution::Init {
                entity_id,
                scheme_registry: &self.schemes,
            },
            Execution::Insert { entity_id, entry } => SchemedExecution::Insert {
                scheme: self.get_scheme(entity_id)?,
                entry,
            },
            Execution::Update {
                entity_id,
                entry,
                condition,
            } => SchemedExecution::Update {
                scheme: self.get_scheme(entity_id)?,
                entry,
                condition,
            },
            Execution::Delete {
                entity_id,
                condition,
            } => SchemedExecution::Delete {
                scheme: self.get_scheme(entity_id)?,
                condition,
            },
        };

        self.language.parse_execution(execution)
    }

    #[cfg(feature = "serde_requests")]
    pub async fn handle_request(&mut self, request: crate::Request) -> crate::Response {
        let (id, inner) = request.into_inner();

        crate::Response::new(
            id,
            match inner {
                crate::RequestInner::Query(query) => {
                    crate::ResponseInner::Query(self.query(query).await)
                }
                crate::RequestInner::Transaction(transaction) => {
                    crate::ResponseInner::Execution(self.execute(transaction).await)
                }
            },
        )
    }
}

/// Query input after an entity ID has been resolved to its schema metadata.
pub struct SchemedQuery<'a, 'b> {
    pub scheme: &'a Scheme,
    pub condition: Option<Condition>,
    pub limit: &'b LimitLike,
    pub offset: &'b LimitLike,
    pub order_by: &'b OrderBy,
}

/// Execution input with schema metadata resolved for the language compiler.
pub enum SchemedExecution<'a> {
    Init {
        entity_id: EntityId,
        scheme_registry: &'a SchemeRegistry,
    },
    Insert {
        scheme: &'a Scheme,
        entry: Entry,
    },
    Update {
        scheme: &'a Scheme,
        entry: Entry,
        condition: Condition,
    },
    Delete {
        scheme: &'a Scheme,
        condition: Condition,
    },
}

/// Dialect-specific SQL plus values to bind to its placeholders.
///
/// The backend must bind `args` in order; values should never be interpolated
/// into the SQL string.
#[derive(Clone)]
pub struct LanguageQuery {
    /// Compiled SQL statement.
    pub query: String,
    /// Parameter values corresponding to statement placeholders.
    pub args: Vec<Value>,
}

impl LanguageQuery {
    /// Construct a compiled statement and its ordered parameter values.
    pub fn new(query: String, args: Vec<Value>) -> Self {
        Self { query, args }
    }
}

impl Debug for LanguageQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "Query: {}\nArgs:  {:?}",
            self.query, self.args
        ))
    }
}

/// Compiles ORM operations into dialect-specific SQL and bound values.
pub trait Language: Default + Sized {
    /// Compile a read query after its entity scheme has been resolved.
    fn parse_single_query(&self, query: SchemedQuery) -> crate::Result<LanguageQuery>;
    /// Compile one write/init operation into one or more statements.
    fn parse_execution(&self, execution: SchemedExecution) -> crate::Result<Vec<LanguageQuery>>;
}

/// Associates a driver with the language implementation it supports.
pub trait Supported<L: Language>: Driver + Sized {
    /// Return backend-specific language state supplied to [`Driver::connect`].
    fn get_language() -> Self::LanguageEnum;
}

/// Unit language configuration for backends that need no extra language state.
pub type DefaultLanguageEnum = ();

/// Async database boundary implemented by a concrete backend driver.
///
/// The core does not choose an executor; the application runs the returned
/// futures. Drivers receive parameterized [`LanguageQuery`] values and must
/// preserve transaction atomicity for `execute`.
pub trait Driver: Sized {
    /// Backend-specific language configuration passed when connecting.
    type LanguageEnum;
    /// Driver-specific connection arguments.
    type ConnectArgs;

    /// Open a database connection or pool.
    fn connect(
        args: Self::ConnectArgs,
        language: Self::LanguageEnum,
    ) -> impl std::future::Future<Output = crate::Result<Self>>;

    /// Execute a parameterized query and return decoded rows.
    fn query(
        &mut self,
        query: LanguageQuery,
    ) -> impl std::future::Future<Output = crate::Result<Vec<Entry>>>;
    /// Execute compiled commands atomically and return affected-row count.
    fn execute(
        &mut self,
        commands: Vec<LanguageQuery>,
    ) -> impl std::future::Future<Output = crate::Result<ExecutionResponse>>;
}
