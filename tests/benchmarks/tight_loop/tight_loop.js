function tightLoop() {
    let i = 0;
    let sum = 0;
    while (i < 10000000) {
        sum = sum + i;
        i = i + 1;
    }
    return sum;
}

const start = performance.now();
const result = tightLoop();
const end = performance.now();
console.log(result);
console.log(`JS Time: ${((end - start) / 1000).toFixed(4)}s`);
