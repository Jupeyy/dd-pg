#![allow(clippy::all)]

pub mod datatypes;
pub mod mt_datatypes;
pub mod mt_pool;
pub mod mt_recycle;
pub mod pool;
pub mod recycle;
pub mod traits;

#[cfg(test)]
mod tests {
    use crate::{
        datatypes::PoolVec as SingleThreadedPoolVec, mt_datatypes::PoolVec as ThreadSafePoolVec,
        mt_pool::Pool as ThreadedSafePool, pool::Pool as SingleThreadedPool,
    };

    #[test]
    fn it_works() {
        let pool = SingleThreadedPool::<Vec<u8>>::with_capacity(10);
        let mut v = pool.new();
        v.push(1);
        // no items in yet
        assert_eq!(pool.items_in_pool(), 0);
        drop(v);
        assert_eq!(pool.items_in_pool(), 1);
    }

    #[test]
    fn it_works_mt() {
        let pool = ThreadedSafePool::<Vec<u8>>::with_capacity(10);
        let mut v = pool.new();
        v.push(1);
        assert_eq!(pool.items_in_pool(), 0);
        drop(v);
        assert_eq!(pool.items_in_pool(), 1);
    }

    #[test]
    fn it_works_capacity() {
        let pool = SingleThreadedPool::<Vec<u8>>::with_sized(10, || Vec::with_capacity(10));
        let v = pool.new();
        assert_eq!(pool.items_in_pool(), 9);
        assert_eq!(v.capacity(), 10);
    }

    #[test]
    fn it_works_capacity_mt() {
        let pool = ThreadedSafePool::<Vec<u8>>::with_sized(10, || Vec::with_capacity(10));
        let v = pool.new();
        assert_eq!(pool.items_in_pool(), 9);
        assert_eq!(v.capacity(), 10);
    }

    fn bench_pool(size: usize) {
        let mut pooled_vec: Vec<SingleThreadedPoolVec<u8>> = Default::default();
        let mut pooled_vec_mt: Vec<ThreadSafePoolVec<u8>> = Default::default();
        let mut vec: Vec<Vec<u8>> = Default::default();
        let mut copy_vec: Vec<u8> = Default::default();
        for i in 0..size {
            copy_vec.push((i % 255) as u8);
        }
        pooled_vec.reserve(size);
        pooled_vec_mt.reserve(size);
        vec.reserve(size);
        let bench = std::time::Instant::now();

        let pool = SingleThreadedPool::<Vec<u8>>::with_sized(size, || Vec::with_capacity(size));
        for _ in 0..size {
            pooled_vec.push(pool.new());
        }
        for i in 0..size {
            pooled_vec[i].clone_from(&copy_vec);
        }

        println!(
            "time st-pool round 1: {}s",
            std::time::Instant::now()
                .duration_since(bench)
                .as_secs_f64()
        );

        let bench = std::time::Instant::now();

        let pool_mt = ThreadedSafePool::<Vec<u8>>::with_sized(size, || Vec::with_capacity(size));
        for _ in 0..size {
            pooled_vec_mt.push(pool_mt.new());
        }
        for i in 0..size {
            pooled_vec_mt[i].clone_from(&copy_vec);
        }

        println!(
            "time mt-pool round 1: {}s",
            std::time::Instant::now()
                .duration_since(bench)
                .as_secs_f64()
        );

        let bench = std::time::Instant::now();

        for _ in 0..size {
            vec.push(Vec::new());
        }
        for i in 0..size {
            vec[i].clone_from(&copy_vec);
        }

        println!(
            "time no-pool round 1: {}s",
            std::time::Instant::now()
                .duration_since(bench)
                .as_secs_f64()
        );

        // #### round 2 #####
        let bench = std::time::Instant::now();
        pooled_vec.clear();
        for _ in 0..size {
            pooled_vec.push(pool.new());
        }
        for i in 0..size {
            pooled_vec[i].clone_from(&copy_vec);
        }

        println!(
            "time st-pool round 2: {}s",
            std::time::Instant::now()
                .duration_since(bench)
                .as_secs_f64()
        );

        let bench = std::time::Instant::now();
        pooled_vec_mt.clear();
        for _ in 0..size {
            pooled_vec_mt.push(pool_mt.new());
        }
        for i in 0..size {
            pooled_vec_mt[i].clone_from(&copy_vec);
        }

        println!(
            "time mt-pool round 2: {}s",
            std::time::Instant::now()
                .duration_since(bench)
                .as_secs_f64()
        );

        let bench = std::time::Instant::now();
        vec.clear();
        for _ in 0..size {
            vec.push(Vec::new());
        }
        for i in 0..size {
            vec[i].clone_from(&copy_vec);
        }

        println!(
            "time no-pool round 2: {}s",
            std::time::Instant::now()
                .duration_since(bench)
                .as_secs_f64()
        );
    }

    #[test]
    fn it_works_bench() {
        println!("size: 4000");
        bench_pool(4000);
        println!("size: 8000");
        bench_pool(8000);
        println!("size: 16000");
        bench_pool(16000);
    }
}
