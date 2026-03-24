import time, math

PI = math.pi
SOLAR_MASS = 4 * PI * PI
DAYS_PER_YEAR = 365.24

BODIES = {
    'sun':     ([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], SOLAR_MASS),
    'jupiter': (
        [4.84143144246472090e+00, -1.16032004402742839e+00, -1.03622044471123109e-01],
        [1.66007664274403694e-03 * DAYS_PER_YEAR, 7.69901118419740425e-03 * DAYS_PER_YEAR, -6.90460016972063023e-05 * DAYS_PER_YEAR],
        9.54791938424326609e-04 * SOLAR_MASS),
    'saturn': (
        [8.34336671824457987e+00, 4.12479856412430479e+00, -4.03523417114321381e-01],
        [-2.76742510726862411e-03 * DAYS_PER_YEAR, 4.99852801234917238e-03 * DAYS_PER_YEAR, 2.30417297573763929e-05 * DAYS_PER_YEAR],
        2.85885980666130812e-04 * SOLAR_MASS),
    'uranus': (
        [1.28943695621391310e+01, -1.51111514016986312e+01, -2.23307578892655734e-01],
        [2.96460137564761618e-03 * DAYS_PER_YEAR, 2.37847173959480950e-03 * DAYS_PER_YEAR, -2.96589568540237556e-05 * DAYS_PER_YEAR],
        4.36624404335156298e-05 * SOLAR_MASS),
    'neptune': (
        [1.53796971148509165e+01, -2.59193146099879641e+01, 1.79258772950371181e-01],
        [2.68067772490389322e-03 * DAYS_PER_YEAR, 1.62824170038242295e-03 * DAYS_PER_YEAR, -9.51592254519715870e-05 * DAYS_PER_YEAR],
        5.15138902046611451e-05 * SOLAR_MASS),
}

def advance(bodies, dt):
    keys = list(bodies.keys())
    for i in range(len(keys)):
        ri, vi, mi = bodies[keys[i]]
        for j in range(i + 1, len(keys)):
            rj, vj, mj = bodies[keys[j]]
            dx = ri[0] - rj[0]; dy = ri[1] - rj[1]; dz = ri[2] - rj[2]
            dist2 = dx*dx + dy*dy + dz*dz
            mag = dt / (dist2 * math.sqrt(dist2))
            vi[0] -= dx * mj * mag; vi[1] -= dy * mj * mag; vi[2] -= dz * mj * mag
            vj[0] += dx * mi * mag; vj[1] += dy * mi * mag; vj[2] += dz * mi * mag
    for key in keys:
        r, v, m = bodies[key]
        r[0] += dt * v[0]; r[1] += dt * v[1]; r[2] += dt * v[2]

def energy(bodies):
    keys = list(bodies.keys())
    e = 0.0
    for i in range(len(keys)):
        ri, vi, mi = bodies[keys[i]]
        e += 0.5 * mi * (vi[0]*vi[0] + vi[1]*vi[1] + vi[2]*vi[2])
        for j in range(i + 1, len(keys)):
            rj, vj, mj = bodies[keys[j]]
            dx = ri[0]-rj[0]; dy = ri[1]-rj[1]; dz = ri[2]-rj[2]
            e -= mi * mj / math.sqrt(dx*dx + dy*dy + dz*dz)
    return e

def offset_momentum(bodies):
    px = py = pz = 0.0
    for key in bodies:
        r, v, m = bodies[key]
        px += v[0]*m; py += v[1]*m; pz += v[2]*m
    r, v, m = bodies['sun']
    v[0] = -px / SOLAR_MASS; v[1] = -py / SOLAR_MASS; v[2] = -pz / SOLAR_MASS

offset_momentum(BODIES)
start = time.time()
for _ in range(50000):
    advance(BODIES, 0.01)
print(f"Energy: {energy(BODIES)}")
end = time.time()
print(f"Python Time: {end - start:.4f}s")
