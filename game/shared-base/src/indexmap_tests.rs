// just some tests to make sure indexmap is good for our use
// everytime one iterates over a hashmap, one actually wants indexmap
#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use base::{
        linked_hash_map_view::LinkedHashMapIterExt,
        system::{System, SystemTimeInterface},
    };
    use hashlink::LinkedHashMap;
    use indexmap::IndexMap;

    #[test]
    fn bench_iteration_hash_queue() {
        let bench_func = |range_max: usize| {
            let mut hm = HashMap::<usize, usize>::new();
            let mut im = IndexMap::<usize, usize>::new();
            let mut std_vec = Vec::<usize>::new();
            let mut lhm = LinkedHashMap::<usize, usize>::new();
            let mut fxhm = rustc_hash::FxHashMap::<usize, usize>::default();

            let mut hm_from = HashMap::<usize, usize>::new();
            let mut im_from = IndexMap::<usize, usize>::new();
            let mut std_vec_from = Vec::<usize>::new();
            let mut lhm_from = LinkedHashMap::<usize, usize>::new();
            let mut fxhm_from = rustc_hash::FxHashMap::<usize, usize>::default();
            hm_from.reserve(range_max);
            im_from.reserve(range_max);
            std_vec_from.reserve(range_max);
            lhm_from.reserve(range_max);
            fxhm_from.reserve(range_max);

            let sys = System::new();

            println!();
            println!("##########################################################################");
            println!("benchmarking with {} elements", range_max);
            println!("#insert");

            let start_time = sys.time_get_nanoseconds();
            (0..range_max).for_each(|i| {
                im.insert(i, i);
            });
            println!(
                "indexmap took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );
            let start_time = sys.time_get_nanoseconds();
            (0..range_max).for_each(|i| {
                hm.insert(i, i);
            });
            println!(
                "hashmap took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );
            let start_time = sys.time_get_nanoseconds();
            (0..range_max).for_each(|i| {
                std_vec.push(i);
            });
            println!(
                "std_vec took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );
            let start_time = sys.time_get_nanoseconds();
            (0..range_max).for_each(|i| {
                lhm.insert(i, i);
            });
            println!(
                "linked-hash-map took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );
            let start_time = sys.time_get_nanoseconds();
            (0..range_max).for_each(|i| {
                fxhm.insert(i, i);
            });
            println!(
                "fx hashmap took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );

            println!();
            println!("#iterate");

            let start_time = sys.time_get_nanoseconds();
            im.iter().for_each(|(_, v)| {
                let _ = std::hint::black_box(v);
            });
            println!(
                "indexmap took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );
            let start_time = sys.time_get_nanoseconds();
            std_vec.iter().for_each(|v| {
                let _ = std::hint::black_box(v);
            });
            println!("vec took {:?}:", sys.time_get_nanoseconds() - start_time);
            let start_time = sys.time_get_nanoseconds();
            hm.iter().for_each(|(_, v)| {
                let _ = std::hint::black_box(v);
            });
            println!(
                "hashmap took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );
            let start_time = sys.time_get_nanoseconds();
            lhm.iter().for_each(|(_, v)| {
                let _ = std::hint::black_box(v);
            });
            println!(
                "linked-hashmap took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );
            let start_time = sys.time_get_nanoseconds();
            fxhm.iter().for_each(|(_, v)| {
                let _ = std::hint::black_box(v);
            });
            println!(
                "fx hashmap took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );
            let start_time = sys.time_get_nanoseconds();
            im.values().for_each(|v| {
                let _ = std::hint::black_box(v);
            });
            println!(
                "indexmap (values only) took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );
            let start_time = sys.time_get_nanoseconds();
            let mut ex_it = LinkedHashMapIterExt::new(&mut lhm);
            ex_it.for_each(|(_, v)| {
                let _ = std::hint::black_box(v);
            });
            println!(
                "linked-hashmap (with view) took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );
            let start_time = sys.time_get_nanoseconds();
            lhm.values().for_each(|v| {
                let _ = std::hint::black_box(v);
            });
            println!(
                "linked-hashmap (values only) took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );
            let start_time = sys.time_get_nanoseconds();
            fxhm.values().for_each(|v| {
                let _ = std::hint::black_box(v);
            });
            println!(
                "fx hashmap (values only) took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );

            println!();
            println!("#access");

            // access (without index)
            let start_time = sys.time_get_nanoseconds();
            for i in 0..range_max {
                std::hint::black_box(im.get(&i).unwrap());
            }
            println!(
                "indexmap took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );
            let start_time = sys.time_get_nanoseconds();
            for i in 0..range_max {
                std::hint::black_box(std_vec.iter().find(|v| **v == i).unwrap());
            }
            println!("vec took {:?}:", sys.time_get_nanoseconds() - start_time);
            let start_time = sys.time_get_nanoseconds();
            for i in 0..range_max {
                std::hint::black_box(std_vec.get(i).unwrap());
            }
            println!(
                "vec (if access by index would be allowed) took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );
            let start_time = sys.time_get_nanoseconds();
            for i in 0..range_max {
                std::hint::black_box(hm.get(&i).unwrap());
            }
            println!(
                "hashmap took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );
            let start_time = sys.time_get_nanoseconds();
            for i in 0..range_max {
                std::hint::black_box(lhm.get(&i).unwrap());
            }
            println!(
                "linked-hashmap took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );
            let start_time = sys.time_get_nanoseconds();
            for i in 0..range_max {
                std::hint::black_box(fxhm.get(&i).unwrap());
            }
            println!(
                "fx hashmap took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );

            println!();
            println!("#clone");

            // push an item to the back
            let start_time = sys.time_get_nanoseconds();
            let im2 = im.clone();
            println!(
                "indexmap took {:?} - {}",
                sys.time_get_nanoseconds() - start_time,
                im2.len()
            );
            let start_time = sys.time_get_nanoseconds();
            let std_vec2 = std_vec.clone();
            println!(
                "vec took {:?} - {}",
                sys.time_get_nanoseconds() - start_time,
                std_vec2.len()
            );
            let start_time = sys.time_get_nanoseconds();
            let hm2 = hm.clone();
            println!(
                "hashmap took {:?} - {}",
                sys.time_get_nanoseconds() - start_time,
                hm2.len()
            );
            let start_time = sys.time_get_nanoseconds();
            let lhm2 = lhm.clone();
            println!(
                "linked-hashmap took {:?} - {}",
                sys.time_get_nanoseconds() - start_time,
                lhm2.len()
            );
            let start_time = sys.time_get_nanoseconds();
            let fxhm2 = fxhm.clone();
            println!(
                "fx hashmap took {:?} - {}",
                sys.time_get_nanoseconds() - start_time,
                fxhm2.len()
            );

            println!();
            println!("#clone_from");

            // push an item to the back
            let start_time = sys.time_get_nanoseconds();
            im_from.clone_from(&im);
            println!(
                "indexmap took {:?} - {}",
                sys.time_get_nanoseconds() - start_time,
                im_from.len()
            );
            let start_time = sys.time_get_nanoseconds();
            std_vec_from.clone_from(&std_vec);
            println!(
                "vec took {:?} - {}",
                sys.time_get_nanoseconds() - start_time,
                std_vec_from.len()
            );
            let start_time = sys.time_get_nanoseconds();
            hm_from.clone_from(&hm);
            println!(
                "hashmap took {:?} - {}",
                sys.time_get_nanoseconds() - start_time,
                hm_from.len()
            );
            let start_time = sys.time_get_nanoseconds();
            lhm_from.clone_from(&lhm);
            println!(
                "linked-hashmap took {:?} - {}",
                sys.time_get_nanoseconds() - start_time,
                lhm_from.len()
            );
            let start_time = sys.time_get_nanoseconds();
            fxhm_from.clone_from(&fxhm);
            println!(
                "fx hashmap took {:?} - {}",
                sys.time_get_nanoseconds() - start_time,
                fxhm_from.len()
            );

            println!();
            println!("#push to back");

            // push an item to the back
            let start_time = sys.time_get_nanoseconds();
            let len = im.len();
            for _i in 0..len {
                im.move_index(0, len - 1);
            }
            println!(
                "indexmap took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );
            let start_time = sys.time_get_nanoseconds();
            let len = std_vec.len();
            for _i in 0..len {
                let el = std_vec.remove(0);
                std_vec.push(el);
            }
            println!("vec took {:?}:", sys.time_get_nanoseconds() - start_time);
            println!("hashmap unsupported");
            let start_time = sys.time_get_nanoseconds();
            let len = lhm.len();
            for i in 0..len {
                lhm.to_back(&i);
            }
            println!(
                "linked-hashmap took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );
            println!("fx hashmap unsupported");

            println!();
            println!("#remove");

            // remove
            let start_time = sys.time_get_nanoseconds();
            for i in 0..range_max {
                im.shift_remove(&i).unwrap();
                if i == range_max - 50 {
                    print!("order check: ");
                    im.values().for_each(|v| print!("{} ", v));
                    println!();
                }
            }
            println!(
                "indexmap took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );
            let start_time = sys.time_get_nanoseconds();
            for i in 0..range_max {
                std_vec.remove(
                    std_vec
                        .iter()
                        .enumerate()
                        .find(|(_, v)| **v == i)
                        .unwrap()
                        .0,
                );
                if i == range_max - 50 {
                    print!("order check: ");
                    std_vec.iter().for_each(|v| print!("{} ", v));
                    println!();
                }
            }
            println!("vec took {:?}:", sys.time_get_nanoseconds() - start_time);
            let start_time = sys.time_get_nanoseconds();
            for i in 0..range_max {
                hm.remove(&i).unwrap();
                if i == range_max - 50 {
                    println!("can't preserve order in hashmap.");
                }
            }
            println!(
                "hashmap took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );
            let start_time = sys.time_get_nanoseconds();
            for i in 0..range_max {
                lhm.remove(&i).unwrap();
                if i == range_max - 50 {
                    print!("order check: ");
                    lhm.values().for_each(|v| print!("{} ", v));
                    println!();
                }
            }
            println!(
                "linked-hashmap took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );
            let start_time = sys.time_get_nanoseconds();
            for i in 0..range_max {
                fxhm.remove(&i).unwrap();
                if i == range_max - 50 {
                    println!("can't preserve order in fx hashmap.");
                }
            }
            println!(
                "fx hashmap took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );
        };
        bench_func(64);
        bench_func(512);
        bench_func(100000);
    }
}
