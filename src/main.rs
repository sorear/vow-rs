extern crate crossbeam;
extern crate rand;

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::hash::{Hash,Hasher,SipHasher};
use std::mem;
use std::sync::Mutex;

fn vow<T,I,D,S,C>(sampler: S,
                distinguished: D,
                iteration_f: I,
                check: C,
                num_threads: u32,
                loop_timeout: usize,
                ) -> (T, T)
                where S : Sync + Fn() -> T,
                      D : Sync + Fn(&T) -> bool,
                      I : Send + Sync + Fn(T) -> T,
                      C : Sync + Send + Fn(T,T) -> bool,
                      T : Eq + Hash + Clone + Send + Sync {

    let mut rv : Option<(T, T)> = None;
    {
        let mut rvp = &mut rv;
        let mut total = 0;

        let mut done = false;
        let mut known : HashMap<T,(T,usize)> = HashMap::new();
        let iteration = &iteration_f;

        let on_dist = Mutex::new(move |mut start: T, dist: T, mut steps: usize| {
            if done { return true; }

            total += steps;
            println!("{}", total);

            let (mut start2, mut steps2) = match known.entry(dist.clone()) {
                Entry::Occupied(o) => o.get().clone(),
                Entry::Vacant(v) => {
                    v.insert((start.clone(), steps));
                    return false;
                }
            };

            if steps > steps2 {
                mem::swap(&mut steps, &mut steps2);
                mem::swap(&mut start, &mut start2);
            }
            //steps <= steps2

            while steps < steps2 {
                start2 = iteration(start2);
                steps2 -= 1;
            }
            //steps == steps2

            if start == start2 {
                println!("Highly unlikely to get here if your function is well behaved");
                return false;
            }

            // steps must be >0 since if steps=0, then start = start2 = dist
            loop {
                let old = start.clone();
                let old2 = start2.clone();
                start = iteration(start);
                start2 = iteration(start2);
                if start == start2 {
                    if check(old.clone(), old2.clone()) {
                        *rvp = Some((old, old2));
                        done = true;
                        return true;
                    } else {
                        return false;
                    }
                }
            }
        });

        let worker = || {
            'outer: loop {
                let mut state = sampler();
                let start = state.clone();
                for step_count in 0 .. loop_timeout {
                    if distinguished(&state) {
                        let mut on_dist_g = on_dist.lock().unwrap();
                        let mut on_dist_f = std::ops::DerefMut::deref_mut(&mut on_dist_g);
                        if on_dist_f(start, state, step_count) {
                        //if (on_dist.lock().unwrap())(start, state, step_count) {
                            break 'outer;
                        }
                        continue 'outer;
                    }
                    state = iteration(state);
                }
                println!("Reached iteration limit (short cycle?)");
            }
        };

        crossbeam::scope(|scope| {
            let mut guards = vec![];
            for _ in 0 .. num_threads {
                guards.push(scope.spawn(&worker));
            }
        });
    }

    rv.unwrap()
}

fn main() {
    let (mut a,mut b) = vow(
        || rand::random::<u64>(),
        |x| (x & 0x1fffff) == 0,
        |x| {
            let mut h = SipHasher::new();
            10u8.hash(&mut h);
            ((x & 1) as usize).hash(&mut h);
            19u8.hash(&mut h);
            3usize.hash(&mut h);
            10u8.hash(&mut h);
            (((x >> 1) & 0x1fffff) as usize).hash(&mut h);
            5u8.hash(&mut h);
            1usize.hash(&mut h);
            10u8.hash(&mut h);
            (((x >> 22) & 0x1fffff) as usize).hash(&mut h);
            5u8.hash(&mut h);
            1usize.hash(&mut h);
            10u8.hash(&mut h);
            (((x >> 43) & 0x1fffff) as usize).hash(&mut h);
            5u8.hash(&mut h);
            1usize.hash(&mut h);
            h.finish()
        },
        |x,y| (x & 1) != (y & 1),
        4,
        20_000_000,
    );

    println!("{:?}",(a,b));
    if (a & 1) != 0 { mem::swap(&mut a,&mut b); }

    println!("[([u8; {}],[u8; {}],[u8; {}]); 0]", (a >> 1) & 0x1fffff, (a >> 22) & 0x1fffff, (a >> 43) & 0x1fffff);
    println!("[([u8; {}],[u8; {}],[u8; {}]); 1]", (b >> 1) & 0x1fffff, (b >> 22) & 0x1fffff, (b >> 43) & 0x1fffff);
}
