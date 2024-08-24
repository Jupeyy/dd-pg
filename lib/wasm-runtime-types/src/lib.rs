use std::cell::RefCell;

use serde::{de::DeserializeOwned, Serialize};
use wasmer::{AsStoreRef, Instance, Memory, StoreMut, StoreRef, TypedFunction};

#[derive(Default, Clone)]
pub struct RawBytesEnv {
    raw_bytes_params: [RefCell<Vec<u8>>; 10],
    raw_bytes_result: RefCell<Vec<u8>>,
    instance: RefCell<Option<InstanceData>>,
}

#[derive(Clone)]
pub struct InstanceData {
    pub result_ptr_ptr: i32,
    pub result_size_ptr: i32,
    pub param_ptr_ptrs: [i32; 10],
    pub param_size_ptrs: [i32; 10],
    pub param_alloc_size_ptrs: [i32; 10],
    pub memory: Memory,
    pub prepare_result_func: TypedFunction<u32, ()>,
}

unsafe impl Send for RawBytesEnv {}
unsafe impl Sync for RawBytesEnv {}

impl RawBytesEnv {
    pub fn param_index_mut(
        &self,
        index: usize,
    ) -> (
        std::cell::RefMut<Vec<u8>>,
        std::cell::Ref<Option<InstanceData>>,
    ) {
        (
            self.raw_bytes_params[index].borrow_mut(),
            self.instance.borrow(),
        )
    }

    pub fn result(&self) -> std::cell::Ref<Vec<u8>> {
        self.raw_bytes_result.borrow()
    }
    pub fn result_mut(&self) -> std::cell::RefMut<Vec<u8>> {
        self.raw_bytes_result.borrow_mut()
    }

    pub fn set_instance(&self, instance: InstanceData) {
        let _ = self.instance.borrow_mut().insert(instance);
    }
}

pub fn read_global_location(
    instance: &Instance,
    store: &mut StoreMut<'_>,
    global_name: &str,
) -> i32 {
    instance
        .exports
        .get_global(global_name)
        .unwrap()
        .get(store)
        .i32()
        .unwrap()
}

pub fn read_global(memory: &wasmer::Memory, store: &StoreRef<'_>, global_ptr: i32) -> i32 {
    let mem_view = memory.view(store);
    let mut result: [u8; std::mem::size_of::<i32>()] = Default::default();
    mem_view.read(global_ptr as u64, &mut result).unwrap();
    // wasm always uses little-endian
    i32::from_le_bytes(result)
}

pub fn write_global(memory: &wasmer::Memory, store: &StoreRef<'_>, global_ptr: i32, data: i32) {
    let mem_view = memory.view(store);
    // wasm always uses little-endian
    mem_view
        .write(global_ptr as u64, &data.to_le_bytes())
        .unwrap();
}

pub fn read_param<F: DeserializeOwned>(
    instance: &InstanceData,
    store: &StoreRef<'_>,
    byte_buffer: &mut Vec<u8>,
    param_index: usize,
) -> F {
    let raw_bytes = byte_buffer;

    let ptr = read_global(
        &instance.memory,
        store,
        instance.param_ptr_ptrs[param_index],
    );
    let size = read_global(
        &instance.memory,
        store,
        instance.param_size_ptrs[param_index],
    ) as usize;

    if size > 1024 * 1024 * 1024 {
        panic!("Currently the memory limit is 1GByte");
    }
    raw_bytes.resize(size, Default::default());

    let mem_view = instance.memory.view(store);
    mem_view.read(ptr as u64, raw_bytes).unwrap();

    bincode::serde::decode_from_slice::<F, _>(
        raw_bytes.as_slice(),
        bincode::config::standard().with_fixed_int_encoding(),
    )
    .unwrap()
    .0
}

pub fn write_result<F: Serialize>(instance: &InstanceData, store: &mut StoreMut<'_>, param: &F) {
    // encode and upload
    let res = bincode::serde::encode_to_vec::<&F, _>(
        param,
        bincode::config::standard().with_fixed_int_encoding(),
    )
    .unwrap();

    instance
        .prepare_result_func
        .call(store, res.len() as u32)
        .unwrap();

    let ptr = read_global(
        &instance.memory,
        &store.as_store_ref(),
        instance.result_ptr_ptr,
    );

    let memory = &instance.memory;
    let mem_view = memory.view(store);
    mem_view.write(ptr as u64, &res).unwrap();
}
