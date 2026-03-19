from abc import ABC, abstractmethod
import numpy as np

BALL_RADIUS = 0.025
HALF_LONG_SIDE = 1.23
HALF_SHORT_SIDE = 0.63


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
    
class TableSurfaceConstraint(Constraint):
    MIN_Y = BALL_RADIUS

    def __init__(self, ball: int):
        self.ball = ball

    def __call__(self, pos: np.ndarray) -> np.floating:
        return np.minimum(pos[self.ball,1] - self.MIN_Y, 0)
    
    def grad(self, pos: np.ndarray) -> np.ndarray:
        grad = np.zeros(pos.shape)
        grad[self.ball,1] = float(pos[self.ball,1] < self.MIN_Y)
        return grad
    
class TableShortSideConstraint(Constraint):
    MAX_X = HALF_SHORT_SIDE - BALL_RADIUS
    MIN_X = - MAX_X

    def __init__(self, ball: int):
        self.ball = ball

    def __call__(self, pos: np.ndarray) -> np.floating:
        return np.minimum(pos[self.ball,0] - self.MIN_X, 0) + np.minimum(self.MAX_X - pos[self.ball,0], 0)
    
    def grad(self, pos: np.ndarray) -> np.ndarray:
        grad = np.zeros(pos.shape)
        grad[self.ball,0] = float(pos[self.ball,0] < self.MIN_X) - float(pos[self.ball,0] > self.MAX_X)
        return grad
    
class TableLongSideConstraint(Constraint):
    MAX_Z = HALF_LONG_SIDE - BALL_RADIUS
    MIN_Z = - MAX_Z

    def __init__(self, ball: int):
        self.ball = ball

    def __call__(self, pos: np.ndarray) -> np.floating:
        return np.minimum(pos[self.ball,2] - self.MIN_Z, 0) + np.minimum(self.MAX_Z - pos[self.ball,2], 0)
    
    def grad(self, pos: np.ndarray) -> np.ndarray:
        grad = np.zeros(pos.shape)
        grad[self.ball,2] = float(pos[self.ball,2] < self.MIN_Z) - float(pos[self.ball,2] > self.MAX_Z)
        return grad
    
def register_distance_constraint(ball1, ball2, constraint_manager):
    L0 = ball1.radius + ball2.radius

    def value(pos: np.ndarray):
        l = np.linalg.norm(pos[1] - pos[0])
        if l < L0:
            return l - L0
        else:
            return np.float64(0.0)
    
    def grad(self, pos: np.ndarray) -> np.ndarray:
        delta: np.ndarray = pos[0] - pos[1]
        l = np.linalg.norm(delta)

        grad = np.zeros(pos.shape)

        if l < self.L0:
            grad[0] = delta / l
            grad[1] = -delta / l

        return grad
    
    constraint_manager.push(
        f"distance({ball1.number}, {ball2.number})",
        [ball1.node.id, ball2.node.id],
        value,
        grad
    )