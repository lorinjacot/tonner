from abc import ABC, abstractmethod

import numpy as np

g = np.array([0.0, 0.0, 0.0])
N = 100

        
class Constraint(ABC):
    @abstractmethod
    def __call__(self, pos: np.ndarray) -> np.floating:
        pass

    @abstractmethod
    def grad(self, pos: np.ndarray) -> np.ndarray:
        pass

    def alpha(self) -> np.floating:
        return np.float64(0.0001)


class DistanceConstraint(Constraint):
    L0 = 0.05

    def __init__(self, obj1: int, obj2: int) -> None:
        self.obj1 = obj1
        self.obj2 = obj2

    def __call__(self, pos: np.ndarray) -> np.floating:
        l =  np.linalg.norm(pos[self.obj2] - pos[self.obj1])
        if l < self.L0:
            return l - self.L0
        else:
            return np.float64(0.0)

    def grad(self, pos: np.ndarray) -> np.ndarray:
        delta: np.ndarray = pos[self.obj1] - pos[self.obj2]
        l = np.linalg.norm(delta)

        grad = np.zeros(pos.shape)

        if l < self.L0:
            grad[self.obj1] = delta / l
            grad[self.obj2] = - delta / l

        return grad


constraints = [
    DistanceConstraint(0, 1),
    DistanceConstraint(0, 2),
    DistanceConstraint(1, 2),
]


def simulate(delta_time: float, balls: list, reset: bool):
    pos = np.stack([ball.node.local_translation for ball in balls])
    vel = np.stack([ball.velocity for ball in balls])

    if reset:
        pos = np.array([
            [0.5, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [-0.06, 0.0, 0.0],
        ])
        vel = np.array([
            [-0.5, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
        ])

    dt = delta_time / N

    for _ in range(N):
        vel = vel + dt * g
        old_pos = pos
        pos = pos + dt * vel

        for c in constraints:
            loss = c(pos)
            if np.abs(loss) > 1e-6:
                grad = c.grad(pos)
                lambd = - loss / (np.sum(np.square(grad)) + c.alpha() / dt**2)
                pos = pos + lambd * grad

        vel = (pos - old_pos) / dt

    for i in range(len(balls)):
        balls[i].node.local_translation = pos[i]
        balls[i].velocity = vel[i]