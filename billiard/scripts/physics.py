from typing import List
import numpy as np
import constraints

g = np.array([0.0, -1.0, 0.0])
drag_coefficient = 0.7
N = 10

C: List[constraints.Constraint] = [
    constraints.TableConstraint(),
]
for i in range(15):
    for j in range(i + 1, 16):
        C.append(constraints.DistanceConstraint(i, j))

BASE_POS = np.array([0.0, 0.025, 0.65])

def f(pos: np.ndarray, vel: np.ndarray) -> np.ndarray:
    f_drag = - drag_coefficient * vel
    return f_drag + g

def simulate(delta_time: float, balls: list, reset: bool):
    pos = np.stack([ball.node.local_translation for ball in balls])
    vel = np.stack([ball.velocity for ball in balls])

    if reset:
        d = 0.05

        dz = np.sqrt(3) / 2 * d
        dx = d

        # white
        b0 = np.array([0.0, 0.025, -0.8])

        # Row 0 (1 ball)
        b1  = np.array([0, 0, 0]) + BASE_POS

        # Row 1 (2 balls)
        b2  = np.array([-dx/2, 0, dz]) + BASE_POS
        b3  = np.array([ dx/2, 0, dz]) + BASE_POS

        # Row 2 (3 balls)
        b4  = np.array([-dx, 0, 2*dz]) + BASE_POS
        b5  = np.array([  0, 0, 2*dz]) + BASE_POS
        b6  = np.array([ dx, 0, 2*dz]) + BASE_POS

        # Row 3 (4 balls)
        b7  = np.array([-3*dx/2, 0, 3*dz]) + BASE_POS
        b8  = np.array([-dx/2,   0, 3*dz]) + BASE_POS
        b9  = np.array([ dx/2,   0, 3*dz]) + BASE_POS
        b10 = np.array([ 3*dx/2, 0, 3*dz]) + BASE_POS

        # Row 4 (5 balls)
        b11 = np.array([-2*dx, 0, 4*dz]) + BASE_POS
        b12 = np.array([-dx,   0, 4*dz]) + BASE_POS
        b13 = np.array([  0,   0, 4*dz]) + BASE_POS
        b14 = np.array([ dx,   0, 4*dz]) + BASE_POS
        b15 = np.array([ 2*dx, 0, 4*dz]) + BASE_POS

        pos = np.array([
            b0,
            b1, b2, b3,
            b4, b5, b6,
            b7, b8, b9, b10,
            b11, b12, b13, b14, b15,
        ])

        vel = np.array([
            [0.0, 0.0, 3.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
        ])

    dt = delta_time / N
    lambdas = np.zeros(len(C))
    alphas = np.array([c.alpha() / dt**2 for c in C])

    for _ in range(N):
        vel = vel + dt * f(pos, vel)
        old_pos = pos
        pos = pos + dt * vel

        for i, c in enumerate(C):
            loss = c(pos)
            if np.abs(loss) > 1e-6:
                grad = c.grad(pos)
                delta_lambda = (- loss - alphas[i] * lambdas[i]) / (np.sum(np.square(grad)) + alphas[i])
                lambdas[i] = lambdas[i] + delta_lambda
                pos = pos + delta_lambda * grad

        vel = (pos - old_pos) / dt

    for i in range(len(balls)):
        balls[i].node.local_translation = pos[i]
        balls[i].velocity = vel[i]