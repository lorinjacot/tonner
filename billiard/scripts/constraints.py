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
        return np.float64(0.0001)


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
    BALL_RADIUS = 0.025

    def __call__(self, pos: np.ndarray) -> np.floating:
        return np.sum(np.minimum(pos[:,1] - self.BALL_RADIUS, 0))

    def grad(self, pos: np.ndarray) -> np.ndarray:
        grad = np.zeros(pos.shape)
        grad[:,1] = pos[:,1] < self.BALL_RADIUS
        return grad
    
    def alpha(self) -> np.floating:
        return np.float64(0.00001)