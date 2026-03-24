import time

def mandelbrot(size, max_iter):
    total = 0
    for y in range(size):
        ci = 2.0 * y / size - 1.0
        for x in range(size):
            cr = 2.0 * x / size - 1.5
            zr = 0.0
            zi = 0.0
            itr = 0
            while itr < max_iter:
                zr2 = zr * zr
                zi2 = zi * zi
                if zr2 + zi2 > 4.0:
                    break
                zi = 2.0 * zr * zi + ci
                zr = zr2 - zi2 + cr
                itr += 1
            total += itr
    return total

start = time.time()
result = mandelbrot(200, 50)
end = time.time()
print(f"Total iterations: {result}")
print(f"Python Time: {end - start:.4f}s")
