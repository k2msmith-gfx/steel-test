//! Minimal Rust + Steel embedding experiment.
//!
//! Mirrors ecl-test but using Steel (Scheme) instead of ECL (Common Lisp).
//! No FFI needed — Steel is pure Rust.

use steel::steel_vm::engine::Engine;
use steel::SteelVal;
use std::time::Instant;

fn bench(label: &str, iters: u32, mut f: impl FnMut()) {
    let start = Instant::now();
    for _ in 0..iters {
        f();
    }
    let elapsed = start.elapsed();
    let per_iter_us = elapsed.as_secs_f64() * 1_000_000.0 / iters as f64;
    println!("{label}: {iters} iters in {elapsed:.2?}  ({per_iter_us:.2} µs/call)");
}

fn main() {
    let mut vm = Engine::new();

    println!("Steel booted\n");

    // Smoke test
    let tests = [
        "(+ 1 2 3)",
        "(map (lambda (x) (+ x 1)) (list 10 20 30))",
        "(define (factorial n) (if (<= n 1) 1 (* n (factorial (- n 1)))))",
        "(factorial 10)",
        r#"(string-append "Hello from Steel embedded in Rust!")"#,
    ];

    for expr in &tests {
        match vm.run(*expr) {
            Ok(results) => {
                for val in &results {
                    println!("{expr}  =>  {val:?}");
                }
            }
            Err(e) => println!("{expr}  =>  ERROR: {e:?}"),
        }
    }

    println!("\n--- benchmarks: parse+eval ---");

    vm.run("(define (factorial n) (if (<= n 1) 1 (* n (factorial (- n 1)))))").unwrap();

    bench("(+ 1 2)         parse+eval", 10_000, || { vm.run("(+ 1 2)").unwrap(); });
    bench("(factorial 20)  parse+eval", 10_000, || { vm.run("(factorial 20)").unwrap(); });

    vm.run("(define (sum-to n) (let loop ((i 0) (acc 0)) (if (>= i n) acc (loop (+ i 1) (+ acc i)))))").unwrap();
    bench("(sum-to 1000)   parse+eval", 1_000,  || { vm.run("(sum-to 1000)").unwrap(); });

    vm.run("(define (tex u v) (* 0.5 (+ 1.0 (sin (+ (* u 6.28318530718) (* v 3.14159265359))))))").unwrap();
    bench("tex(u,v)        parse+eval", 10_000, || { vm.run("(tex 0.5 0.3)").unwrap(); });

    vm.run("(define (dot-cos ax ay az bx by bz) (cos (+ (* ax bx) (* ay by) (* az bz))))").unwrap();
    bench("dot-cos         parse+eval", 10_000, || { vm.run("(dot-cos 0.6 0.8 0.0 0.8 0.6 0.0)").unwrap(); });

    println!("\n--- benchmarks: compile-once, run-only ---");

    let add_prog = vm.emit_raw_program_no_path("(+ 1 2)").unwrap();
    let fac_prog = vm.emit_raw_program_no_path("(factorial 20)").unwrap();
    let sum_prog = vm.emit_raw_program_no_path("(sum-to 1000)").unwrap();
    let tex_prog = vm.emit_raw_program_no_path("(tex 0.5 0.3)").unwrap();
    let dc_prog  = vm.emit_raw_program_no_path("(dot-cos 0.6 0.8 0.0 0.8 0.6 0.0)").unwrap();

    bench("(+ 1 2)         run-only", 10_000, || { vm.run_raw_program(add_prog.clone()).unwrap(); });
    bench("(factorial 20)  run-only", 10_000, || { vm.run_raw_program(fac_prog.clone()).unwrap(); });
    bench("(sum-to 1000)   run-only", 1_000,  || { vm.run_raw_program(sum_prog.clone()).unwrap(); });
    bench("tex(u,v)        run-only", 10_000, || { vm.run_raw_program(tex_prog.clone()).unwrap(); });
    bench("dot-cos         run-only", 10_000, || { vm.run_raw_program(dc_prog.clone()).unwrap();  });

    println!("\n--- benchmarks: call_function_by_name_with_args (v3) ---");

    let u = SteelVal::NumV(0.5);
    let v = SteelVal::NumV(0.3);

    bench("tex(u,v)  call_fn_by_name ", 100_000, || {
        vm.call_function_by_name_with_args("tex", vec![u.clone(), v.clone()]).unwrap();
    });

    let ax = SteelVal::NumV(0.6);
    let ay = SteelVal::NumV(0.8);
    let az = SteelVal::NumV(0.0);
    let bx = SteelVal::NumV(0.8);
    let by = SteelVal::NumV(0.6);
    let bz = SteelVal::NumV(0.0);

    bench("dot-cos  call_fn_by_name ", 100_000, || {
        vm.call_function_by_name_with_args(
            "dot-cos",
            vec![ax.clone(), ay.clone(), az.clone(), bx.clone(), by.clone(), bz.clone()],
        ).unwrap();
    });

    println!("\n--- benchmarks: extract_value + call_function_with_args_from_mut_slice (v4) ---");

    // Pre-resolve named functions once; call_function_with_args_from_mut_slice
    // skips string lookup and Vec allocation on each call.
    vm.run("(define (badd) (+ 1 2))").unwrap();
    let fn_add = vm.extract_value("badd").expect("badd not defined");
    let fn_fac = vm.extract_value("factorial").expect("factorial not defined");
    let fn_sum = vm.extract_value("sum-to").expect("sum-to not defined");
    let fn_tex = vm.extract_value("tex").expect("tex not defined");
    let fn_dc  = vm.extract_value("dot-cos").expect("dot-cos not defined");

    let arg20   = SteelVal::IntV(20);
    let arg1000 = SteelVal::IntV(1000);

    bench("(+ 1 2)         call-fn  ", 100_000, || {
        vm.call_function_with_args_from_mut_slice(fn_add.clone(), &mut []).unwrap();
    });
    bench("(factorial 20)  call-fn  ", 100_000, || {
        vm.call_function_with_args_from_mut_slice(fn_fac.clone(), &mut [arg20.clone()]).unwrap();
    });
    bench("(sum-to 1000)   call-fn  ",  10_000, || {
        vm.call_function_with_args_from_mut_slice(fn_sum.clone(), &mut [arg1000.clone()]).unwrap();
    });
    bench("tex(u,v)        call-fn  ", 100_000, || {
        vm.call_function_with_args_from_mut_slice(fn_tex.clone(), &mut [u.clone(), v.clone()]).unwrap();
    });
    bench("dot-cos         call-fn  ", 100_000, || {
        let mut args = [ax.clone(), ay.clone(), az.clone(), bx.clone(), by.clone(), bz.clone()];
        vm.call_function_with_args_from_mut_slice(fn_dc.clone(), &mut args).unwrap();
    });

    let result = vm.call_function_with_args_from_mut_slice(fn_dc.clone(), &mut [
        ax.clone(), ay.clone(), az.clone(), bx.clone(), by.clone(), bz.clone(),
    ]).unwrap();
    println!("\ndot-cos(0.6,0.8,0, 0.8,0.6,0) = {result:?}");
}
