import numpy as np
from numpy.typing import NDArray

g = np.array([0.0, 0.0, 0.0])
n = 100

def simulate(delta_time: float, balls: list, reset: bool):
    pos = np.stack([ball.node.local_translation for ball in balls])
    vel = np.stack([ball.velocity for ball in balls])

    if reset:
        pos = np.array([
            [0.3, 0.0, 0.0],
            [0.15, 0.0, 0.0],
            [0.0, 0.0, 0.0],
        ])
        vel = np.array([
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.3, 0.0, 0.0],
        ])

    dt = delta_time / n

    for _ in range(n):
        vel = vel + dt * g
        old_pos = pos
        pos = pos + dt * vel

        constraints = [
            DistanceConstraint(0, 1, pos),
            DistanceConstraint(0, 2, pos),
            DistanceConstraint(1, 2, pos),
        ]
        for c in constraints:
            for i in range(len(balls)):
                pos[i] += c.delta_pos(i)

        vel = (pos - old_pos) / dt

    for i in range(len(balls)):
        balls[i].node.local_translation = pos[i]
        balls[i].velocity = vel[i]

class DistanceConstraint:
    MIN_DISTANCE = 0.05

    def __init__(self, obj1: int, obj2: int, pos: NDArray[np.float64]) -> None:
        self.obj1 = obj1
        self.obj2 = obj2
        delta = pos[obj2] - pos[obj1]
        l = np.linalg.norm(delta)
        if l < self.MIN_DISTANCE:
            self.factor = (l - self.MIN_DISTANCE) * (delta / l)
        else:
            self.factor = np.zeros(3)

    def delta_pos(self, obj: int) -> NDArray[np.float64]:
        if obj == self.obj1:
            return 0.01 * 0.5 * self.factor
        elif obj == self.obj2:
            return - 0.01 * 0.5 * self.factor
        else:
            return np.zeros(3)