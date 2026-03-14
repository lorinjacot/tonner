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
        """compliance (inverse stiffness)"""
        return np.float64(1e-6)


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

    def __init__(self, obj: int):
        self.obj = obj

    def __call__(self, pos: np.ndarray) -> np.floating:
        return np.float64(
            np.minimum(self.MAX_X - pos[self.obj,0], 0)
            + np.minimum(pos[self.obj,0] - self.MIN_X, 0)
            + np.minimum(pos[self.obj,1] - self.MIN_Y, 0)
            + np.minimum(self.MAX_Z - pos[self.obj,2], 0)
            + np.minimum(pos[self.obj,2] - self.MIN_Z, 0)
        )

    def grad(self, pos: np.ndarray) -> np.ndarray:
        xs = pos[self.obj,0]
        ys = pos[self.obj,1]
        zs = pos[self.obj,2]

        grad = np.zeros(pos.shape)
        if xs > self.MAX_X:
            grad[self.obj,0] = - 1.0
        elif xs < self.MIN_X:
            grad[self.obj,0] = 1.0
            
        if ys < self.MIN_Y:
            grad[self.obj,1] = 1.0

        if zs > self.MAX_Z:
            grad[self.obj,2] = - 1.0
        elif zs < self.MIN_Z:
            grad[self.obj,2] = 1.0
            
        return grad