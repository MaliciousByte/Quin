const PI = Math.PI;
const SOLAR_MASS = 4 * PI * PI;
const DAYS_PER_YEAR = 365.24;

function Body(x, y, z, vx, vy, vz, mass) {
    return [x, y, z, vx * DAYS_PER_YEAR, vy * DAYS_PER_YEAR, vz * DAYS_PER_YEAR, mass * SOLAR_MASS];
}

const bodies = [
    [0, 0, 0, 0, 0, 0, SOLAR_MASS], // Sun
    Body(4.84143144246472090e+00, -1.16032004402742839e+00, -1.03622044471123109e-01,
         1.66007664274403694e-03, 7.69901118419740425e-03, -6.90460016972063023e-05,
         9.54791938424326609e-04),
    Body(8.34336671824457987e+00, 4.12479856412430479e+00, -4.03523417114321381e-01,
         -2.76742510726862411e-03, 4.99852801234917238e-03, 2.30417297573763929e-05,
         2.85885980666130812e-04),
    Body(1.28943695621391310e+01, -1.51111514016986312e+01, -2.23307578892655734e-01,
         2.96460137564761618e-03, 2.37847173959480950e-03, -2.96589568540237556e-05,
         4.36624404335156298e-05),
    Body(1.53796971148509165e+01, -2.59193146099879641e+01, 1.79258772950371181e-01,
         2.68067772490389322e-03, 1.62824170038242295e-03, -9.51592254519715870e-05,
         5.15138902046611451e-05),
];

function offsetMomentum() {
    let px = 0, py = 0, pz = 0;
    for (let i = 0; i < bodies.length; i++) {
        const b = bodies[i];
        px += b[3] * b[6]; py += b[4] * b[6]; pz += b[5] * b[6];
    }
    bodies[0][3] = -px / SOLAR_MASS;
    bodies[0][4] = -py / SOLAR_MASS;
    bodies[0][5] = -pz / SOLAR_MASS;
}

function advance(dt) {
    const n = bodies.length;
    for (let i = 0; i < n; i++) {
        const bi = bodies[i];
        for (let j = i + 1; j < n; j++) {
            const bj = bodies[j];
            const dx = bi[0] - bj[0], dy = bi[1] - bj[1], dz = bi[2] - bj[2];
            const dist2 = dx*dx + dy*dy + dz*dz;
            const mag = dt / (dist2 * Math.sqrt(dist2));
            bi[3] -= dx * bj[6] * mag; bi[4] -= dy * bj[6] * mag; bi[5] -= dz * bj[6] * mag;
            bj[3] += dx * bi[6] * mag; bj[4] += dy * bi[6] * mag; bj[5] += dz * bi[6] * mag;
        }
    }
    for (let i = 0; i < n; i++) {
        const b = bodies[i];
        b[0] += dt * b[3]; b[1] += dt * b[4]; b[2] += dt * b[5];
    }
}

function energy() {
    let e = 0;
    const n = bodies.length;
    for (let i = 0; i < n; i++) {
        const bi = bodies[i];
        e += 0.5 * bi[6] * (bi[3]*bi[3] + bi[4]*bi[4] + bi[5]*bi[5]);
        for (let j = i + 1; j < n; j++) {
            const bj = bodies[j];
            const dx = bi[0]-bj[0], dy = bi[1]-bj[1], dz = bi[2]-bj[2];
            e -= bi[6] * bj[6] / Math.sqrt(dx*dx + dy*dy + dz*dz);
        }
    }
    return e;
}

offsetMomentum();
const start = performance.now();
for (let i = 0; i < 50000; i++) advance(0.01);
const end = performance.now();
console.log(`Energy: ${energy()}`);
console.log(`JS Time: ${((end - start) / 1000).toFixed(4)}s`);
