from abc import ABC, abstractmethod
import numpy as np


class Constraint(ABC):
    @abstractmethod
    def __call__(self, pos: np.ndarray) -> np.floating:
        pass

    @abstractmethod
    def grad(self, pos: np.ndarray) -> np.ndarray:
        pass

    def alpha(self) -> np.floating:
        return np.float64(0.00001)


class DistanceConstraint(Constraint):
    L0 = 0.05

    def __init__(self, obj1: int, obj2: int) -> None:
        self.obj1 = obj1
        self.obj2 = obj2

    def __call__(self, pos: np.ndarray) -> np.floating:
        l = np.linalg.norm(pos[self.obj2] - pos[self.obj1])
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
            grad[self.obj2] = -delta / l

        return grad


class TableConstraint(Constraint):
    HALF_LONG_SIDE = 1.23
    HALF_SHORT_SIDE = 0.63
    BALL_RADIUS = 0.025

    MAX_X = HALF_SHORT_SIDE - BALL_RADIUS
    MIN_X = - MAX_X

    MIN_Y = BALL_RADIUS

    MAX_Z = HALF_LONG_SIDE - BALL_RADIUS
    MIN_Z = - MAX_Z

    def __call__(self, pos: np.ndarray) -> np.floating:
        return (
            np.sum(np.minimum(self.MAX_X - pos[:,0], 0))
            + np.sum(np.minimum(pos[:,0] - self.MIN_X, 0))
            + np.sum(np.minimum(pos[:,1] - self.MIN_Y, 0))
            + np.sum(np.minimum(self.MAX_Z - pos[:,2], 0))
            + np.sum(np.minimum(pos[:,2] - self.MIN_Z, 0))
        )

    def grad(self, pos: np.ndarray) -> np.ndarray:
        xs = pos[:,0]
        ys = pos[:,1]
        zs = pos[:,2]

        grad = np.zeros(pos.shape)
        grad[:,0][xs > self.MAX_X] = - 1.0
        grad[:,0][xs < self.MIN_X] = 1.0
        grad[:,1] = ys < self.MIN_Y
        grad[:,2][zs > self.MAX_Z] = - 1.0
        grad[:,2][zs < self.MIN_Z] = 1.0
        return grad
    
    def alpha(self) -> np.floating:
        return np.float64(0.00001)