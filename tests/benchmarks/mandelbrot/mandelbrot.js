function mandelbrot(size, maxIter) {
    let total = 0;
    for (let y = 0; y < size; y++) {
        const ci = 2.0 * y / size - 1.0;
        for (let x = 0; x < size; x++) {
            const cr = 2.0 * x / size - 1.5;
            let zr = 0.0, zi = 0.0;
            let iter = 0;
            while (iter < maxIter) {
                const zr2 = zr * zr;
                const zi2 = zi * zi;
                if (zr2 + zi2 > 4.0) break;
                zi = 2.0 * zr * zi + ci;
                zr = zr2 - zi2 + cr;
                iter++;
            }
            total += iter;
        }
    }
    return total;
}

const start = performance.now();
const result = mandelbrot(200, 50);
const end = performance.now();
console.log(`Total iterations: ${result}`);
console.log(`JS Time: ${((end - start) / 1000).toFixed(4)}s`);
