// just some tests to make sure indexmap is good for our use
// everytime one iterates over a hashmap, one actually wants indexmap
#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use base::system::{System, SystemTimeInterface};
    use hashlink::LinkedHashMap;
    use indexmap::IndexMap;

    #[test]
    fn bench_iteration_hash_queue() {
        let bench_func = |range_max: usize| {
            let mut hm = HashMap::<usize, usize>::new();
            let mut im = IndexMap::<usize, usize>::new();
            let mut std_vec = Vec::<usize>::new();
            let mut lhm = LinkedHashMap::<usize, usize>::new();
            let sys = System::new();

            println!("");
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

            println!("");
            println!("#iterate");

            let start_time = sys.time_get_nanoseconds();
            im.iter().for_each(|(_, v)| {
                let _ = v;
            });
            println!(
                "indexmap took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );
            let start_time = sys.time_get_nanoseconds();
            std_vec.iter().for_each(|v| {
                let _ = v;
            });
            println!("vec took {:?}:", sys.time_get_nanoseconds() - start_time);
            let start_time = sys.time_get_nanoseconds();
            hm.iter().for_each(|(_, v)| {
                let _ = v;
            });
            println!(
                "hashmap took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );
            let start_time = sys.time_get_nanoseconds();
            lhm.iter().for_each(|(_, v)| {
                let _ = v;
            });
            println!(
                "linked-hashmap took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );
            let start_time = sys.time_get_nanoseconds();
            im.values().for_each(|v| {
                let _ = v;
            });
            println!(
                "indexmap (values only) took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );

            println!("");
            println!("#access");

            // access (without index)
            let start_time = sys.time_get_nanoseconds();
            for i in 0..range_max {
                im.get(&i).unwrap();
            }
            println!(
                "indexmap took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );
            let start_time = sys.time_get_nanoseconds();
            for i in 0..range_max {
                std_vec.iter().find(|v| **v == i).unwrap();
            }
            println!("vec took {:?}:", sys.time_get_nanoseconds() - start_time);
            let start_time = sys.time_get_nanoseconds();
            for i in 0..range_max {
                hm.get(&i).unwrap();
            }
            println!(
                "hashmap took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );
            let start_time = sys.time_get_nanoseconds();
            for i in 0..range_max {
                lhm.get(&i).unwrap();
            }
            println!(
                "linked-hashmap took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );

            println!("");
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

            println!("");
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

            println!("");
            println!("#remove");

            // remove
            let start_time = sys.time_get_nanoseconds();
            for i in 0..range_max {
                im.shift_remove(&i).unwrap();
                if i == range_max - 50 {
                    print!("order check: ");
                    im.values().for_each(|v| print!("{} ", v));
                    println!("");
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
                    println!("");
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
                    println!("");
                }
            }
            println!(
                "linked-hashmap took {:?}:",
                sys.time_get_nanoseconds() - start_time
            );
        };
        bench_func(64);
        bench_func(512);
        bench_func(100000);
    }
}
