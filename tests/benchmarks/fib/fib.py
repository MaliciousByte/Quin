import time

def fib(n):
    if n < 2:
        return n
    return fib(n - 1) + fib(n - 2)

t0 = time.time()
res = fib(35)
t1 = time.time()

print(res)
print("Python Time: " + str(t1 - t0) + "s")
