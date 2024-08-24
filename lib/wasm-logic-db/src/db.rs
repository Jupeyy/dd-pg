use std::collections::{BTreeMap, HashSet};
use std::{cell::RefCell, collections::HashMap, sync::Arc};

use base_io::io_batcher::{IoBatcher, IoBatcherTask};
use game_database::{statement::QueryProperties, traits::DbInterface};
use game_database::{statement::StatementDriverProps, traits::DbKind, types::DbType};
use wasm_runtime_types::{read_param, write_result, RawBytesEnv};
use wasmer::{imports, AsStoreRef, Function, FunctionEnv, FunctionEnvMut, Imports, Store};

pub struct WasmDatabaseLogicImpl {
    pub io_batcher: IoBatcher,
    pub db: Arc<dyn DbInterface>,
    setup_tasks: RefCell<HashMap<u64, IoBatcherTask<()>>>,
    prepare_tasks: RefCell<HashMap<u64, IoBatcherTask<()>>>,
    fetch_tasks_optional: RefCell<HashMap<u64, IoBatcherTask<Option<HashMap<String, DbType>>>>>,
    fetch_tasks_one: RefCell<HashMap<u64, IoBatcherTask<HashMap<String, DbType>>>>,
    fetch_tasks_all: RefCell<HashMap<u64, IoBatcherTask<Vec<HashMap<String, DbType>>>>>,
}

impl WasmDatabaseLogicImpl {
    fn new(io_batcher: IoBatcher, db: Arc<dyn DbInterface>) -> Self {
        Self {
            io_batcher,
            db,
            setup_tasks: Default::default(),
            prepare_tasks: Default::default(),
            fetch_tasks_optional: Default::default(),
            fetch_tasks_one: Default::default(),
            fetch_tasks_all: Default::default(),
        }
    }

    fn kinds(&self) -> HashSet<DbKind> {
        self.db.kinds()
    }

    fn setup(
        &self,
        id: u64,
        version_name: String,
        versioned_stmts: BTreeMap<i64, Vec<String>>,
    ) -> Option<Result<(), String>> {
        let mut tasks = self.setup_tasks.borrow_mut();
        match tasks.get(&id) {
            Some(task) => {
                if task.is_finished() {
                    let task = tasks.remove(&id).unwrap();
                    Some(task.get_storage().map_err(|err| err.to_string()))
                } else {
                    None
                }
            }
            None => {
                let db = self.db.clone();
                let task = self
                    .io_batcher
                    .spawn(async move { db.setup(&version_name, versioned_stmts).await });
                tasks.insert(id, task);
                None
            }
        }
    }

    fn prepare_statement(
        &self,
        id: u64,
        unique_id: String,
        query_props: QueryProperties,
        driver_props: HashMap<DbKind, StatementDriverProps>,
    ) -> Option<Result<(), String>> {
        let mut tasks = self.prepare_tasks.borrow_mut();
        match tasks.get(&id) {
            Some(task) => {
                if task.is_finished() {
                    let task = tasks.remove(&id).unwrap();
                    Some(task.get_storage().map_err(|err| err.to_string()))
                } else {
                    None
                }
            }
            None => {
                let db = self.db.clone();
                let task = self.io_batcher.spawn(async move {
                    db.prepare_statement(&unique_id, &query_props, &driver_props)
                        .await
                });
                tasks.insert(id, task);
                None
            }
        }
    }

    fn drop_statement(&self, unique_id: String) {
        self.db.drop_statement(&unique_id);
    }

    fn fetch_optional(
        &self,
        id: u64,
        unique_id: String,
        args: Vec<DbType>,
    ) -> Option<Result<Option<HashMap<String, DbType>>, String>> {
        let mut tasks = self.fetch_tasks_optional.borrow_mut();
        match tasks.get(&id) {
            Some(task) => {
                if task.is_finished() {
                    let task = tasks.remove(&id).unwrap();
                    Some(task.get_storage().map_err(|err| err.to_string()))
                } else {
                    None
                }
            }
            None => {
                let db = self.db.clone();
                let task = self
                    .io_batcher
                    .spawn(async move { db.fetch_optional(&unique_id, args).await });
                tasks.insert(id, task);
                None
            }
        }
    }

    fn fetch_one(
        &self,
        id: u64,
        unique_id: String,
        args: Vec<DbType>,
    ) -> Option<Result<HashMap<String, DbType>, String>> {
        let mut tasks = self.fetch_tasks_one.borrow_mut();
        match tasks.get(&id) {
            Some(task) => {
                if task.is_finished() {
                    let task = tasks.remove(&id).unwrap();
                    Some(task.get_storage().map_err(|err| err.to_string()))
                } else {
                    None
                }
            }
            None => {
                let db = self.db.clone();
                let task = self
                    .io_batcher
                    .spawn(async move { db.fetch_one(&unique_id, args).await });
                tasks.insert(id, task);
                None
            }
        }
    }

    fn fetch_all(
        &self,
        id: u64,
        unique_id: String,
        args: Vec<DbType>,
    ) -> Option<Result<Vec<HashMap<String, DbType>>, String>> {
        let mut tasks = self.fetch_tasks_all.borrow_mut();
        match tasks.get(&id) {
            Some(task) => {
                if task.is_finished() {
                    let task = tasks.remove(&id).unwrap();
                    Some(task.get_storage().map_err(|err| err.to_string()))
                } else {
                    None
                }
            }
            None => {
                let db = self.db.clone();
                let task = self
                    .io_batcher
                    .spawn(async move { db.fetch_all(&unique_id, args).await });
                tasks.insert(id, task);
                None
            }
        }
    }
}

unsafe impl Send for WasmDatabaseLogicImpl {}
unsafe impl Sync for WasmDatabaseLogicImpl {}

pub struct WasmDatabaseLogic(pub Arc<WasmDatabaseLogicImpl>);

impl WasmDatabaseLogic {
    pub fn new(io_batcher: IoBatcher, db: Arc<dyn DbInterface>) -> Self {
        Self(Arc::new(WasmDatabaseLogicImpl::new(io_batcher, db)))
    }

    pub fn get_wasm_logic_imports(
        &self,
        store: &mut Store,
        raw_bytes_env: &FunctionEnv<Arc<RawBytesEnv>>,
    ) -> Imports {
        fn kinds(
            logic_clone: &Arc<WasmDatabaseLogicImpl>,
            mut env: FunctionEnvMut<Arc<RawBytesEnv>>,
        ) {
            let (data, mut store) = env.data_and_store_mut();
            let (_, instance) = data.param_index_mut(0);

            let res = logic_clone.kinds();
            write_result(instance.as_ref().unwrap(), &mut store, &res);
        }

        fn setup(
            logic_clone: &Arc<WasmDatabaseLogicImpl>,
            mut env: FunctionEnvMut<Arc<RawBytesEnv>>,
        ) {
            let (data, mut store) = env.data_and_store_mut();
            let (mut param0, instance) = data.param_index_mut(0);
            let id: u64 = read_param(
                instance.as_ref().unwrap(),
                &store.as_store_ref(),
                &mut param0,
                0,
            );
            let version_name: String = read_param(
                instance.as_ref().unwrap(),
                &store.as_store_ref(),
                &mut param0,
                1,
            );
            let versioned_stmts: BTreeMap<i64, Vec<String>> = read_param(
                instance.as_ref().unwrap(),
                &store.as_store_ref(),
                &mut param0,
                2,
            );

            let res = logic_clone.setup(id, version_name, versioned_stmts);
            write_result(instance.as_ref().unwrap(), &mut store, &res);
        }

        fn prepare_statement(
            logic_clone: &Arc<WasmDatabaseLogicImpl>,
            mut env: FunctionEnvMut<Arc<RawBytesEnv>>,
        ) {
            let (data, mut store) = env.data_and_store_mut();
            let (mut param0, instance) = data.param_index_mut(0);
            let id: u64 = read_param(
                instance.as_ref().unwrap(),
                &store.as_store_ref(),
                &mut param0,
                0,
            );
            let unique_id: String = read_param(
                instance.as_ref().unwrap(),
                &store.as_store_ref(),
                &mut param0,
                1,
            );
            let query_props: QueryProperties = read_param(
                instance.as_ref().unwrap(),
                &store.as_store_ref(),
                &mut param0,
                2,
            );
            let driver_props: HashMap<DbKind, StatementDriverProps> = read_param(
                instance.as_ref().unwrap(),
                &store.as_store_ref(),
                &mut param0,
                3,
            );

            let res = logic_clone.prepare_statement(id, unique_id, query_props, driver_props);
            write_result(instance.as_ref().unwrap(), &mut store, &res);
        }

        fn drop_statement(
            logic_clone: &Arc<WasmDatabaseLogicImpl>,
            mut env: FunctionEnvMut<Arc<RawBytesEnv>>,
        ) {
            let (data, mut store) = env.data_and_store_mut();
            let (mut param0, instance) = data.param_index_mut(0);
            let unique_id: String = read_param(
                instance.as_ref().unwrap(),
                &store.as_store_ref(),
                &mut param0,
                0,
            );

            logic_clone.drop_statement(unique_id);
        }

        fn fetch_optional(
            logic_clone: &Arc<WasmDatabaseLogicImpl>,
            mut env: FunctionEnvMut<Arc<RawBytesEnv>>,
        ) {
            let (data, mut store) = env.data_and_store_mut();
            let (mut param0, instance) = data.param_index_mut(0);
            let id: u64 = read_param(
                instance.as_ref().unwrap(),
                &store.as_store_ref(),
                &mut param0,
                0,
            );
            let unique_id: String = read_param(
                instance.as_ref().unwrap(),
                &store.as_store_ref(),
                &mut param0,
                1,
            );
            let args: Vec<DbType> = read_param(
                instance.as_ref().unwrap(),
                &store.as_store_ref(),
                &mut param0,
                2,
            );

            let res = logic_clone.fetch_optional(id, unique_id, args);
            write_result(instance.as_ref().unwrap(), &mut store, &res);
        }

        fn fetch_one(
            logic_clone: &Arc<WasmDatabaseLogicImpl>,
            mut env: FunctionEnvMut<Arc<RawBytesEnv>>,
        ) {
            let (data, mut store) = env.data_and_store_mut();
            let (mut param0, instance) = data.param_index_mut(0);
            let id: u64 = read_param(
                instance.as_ref().unwrap(),
                &store.as_store_ref(),
                &mut param0,
                0,
            );
            let unique_id: String = read_param(
                instance.as_ref().unwrap(),
                &store.as_store_ref(),
                &mut param0,
                1,
            );
            let args: Vec<DbType> = read_param(
                instance.as_ref().unwrap(),
                &store.as_store_ref(),
                &mut param0,
                2,
            );

            let res = logic_clone.fetch_one(id, unique_id, args);
            write_result(instance.as_ref().unwrap(), &mut store, &res);
        }

        fn fetch_all(
            logic_clone: &Arc<WasmDatabaseLogicImpl>,
            mut env: FunctionEnvMut<Arc<RawBytesEnv>>,
        ) {
            let (data, mut store) = env.data_and_store_mut();
            let (mut param0, instance) = data.param_index_mut(0);
            let id: u64 = read_param(
                instance.as_ref().unwrap(),
                &store.as_store_ref(),
                &mut param0,
                0,
            );
            let unique_id: String = read_param(
                instance.as_ref().unwrap(),
                &store.as_store_ref(),
                &mut param0,
                1,
            );
            let args: Vec<DbType> = read_param(
                instance.as_ref().unwrap(),
                &store.as_store_ref(),
                &mut param0,
                2,
            );

            let res = logic_clone.fetch_all(id, unique_id, args);
            write_result(instance.as_ref().unwrap(), &mut store, &res);
        }

        let logic = self.0.clone();
        let logic1 = self.0.clone();
        let logic2 = self.0.clone();
        let logic3 = self.0.clone();
        let logic4 = self.0.clone();
        let logic5 = self.0.clone();
        let logic6 = self.0.clone();

        imports! {
            "env" => {
                "api_db_kinds" => Function::new_typed_with_env(store, raw_bytes_env, move |env: FunctionEnvMut<Arc<RawBytesEnv>>| kinds(&logic, env)),
                "api_db_setup" => Function::new_typed_with_env(store, raw_bytes_env, move |env: FunctionEnvMut<Arc<RawBytesEnv>>| setup(&logic1, env)),
                "api_db_prepare_statement" => Function::new_typed_with_env(store, raw_bytes_env, move |env: FunctionEnvMut<Arc<RawBytesEnv>>| prepare_statement(&logic2, env)),
                "api_db_drop_statement" => Function::new_typed_with_env(store, raw_bytes_env, move |env: FunctionEnvMut<Arc<RawBytesEnv>>| drop_statement(&logic3, env)),
                "api_db_fetch_optional" => Function::new_typed_with_env(store, raw_bytes_env, move |env: FunctionEnvMut<Arc<RawBytesEnv>>| fetch_optional(&logic4, env)),
                "api_db_fetch_one" => Function::new_typed_with_env(store, raw_bytes_env, move |env: FunctionEnvMut<Arc<RawBytesEnv>>| fetch_one(&logic5, env)),
                "api_db_fetch_all" => Function::new_typed_with_env(store, raw_bytes_env, move |env: FunctionEnvMut<Arc<RawBytesEnv>>| fetch_all(&logic6, env)),
            }
        }
    }
}
