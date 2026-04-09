function fib(n) {
    if (n < 2) return n;
    return fib(n - 1) + fib(n - 2);
}

let t0 = performance.now();
let res = fib(35);
let t1 = performance.now();

console.log(res);
console.log("Time: " + ((t1 - t0) / 1000) + "s");
