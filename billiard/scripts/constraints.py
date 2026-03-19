import numpy as np

BALL_RADIUS = 0.025
HALF_LONG_SIDE = 1.23
HALF_SHORT_SIDE = 0.63
    
def register_distance_constraint(ball1, ball2, constraint_manager):
    L0 = ball1.radius + ball2.radius

    def value(pos: np.ndarray):
        l = np.linalg.norm(pos[1] - pos[0])
        if l < L0:
            return l - L0
        else:
            return np.float64(0.0)
    
    def grad(pos: np.ndarray) -> np.ndarray:
        delta = pos[0] - pos[1]
        l = np.linalg.norm(delta)

        grad = np.zeros(pos.shape)

        if l < L0:
            grad[0] = delta / l
            grad[1] = -delta / l

        return grad
    
    constraint_manager.push(
        f"distance({ball1.number}, {ball2.number})",
        [ball1.node.id, ball2.node.id],
        value,
        grad,
    )

def register_table_surface_constraint(ball, constraint_manager):
    MIN_Y = BALL_RADIUS

    def value(pos: np.ndarray) -> np.floating:
        return np.minimum(pos[0,1] - MIN_Y, 0)
    
    def grad(pos: np.ndarray) -> np.ndarray:
        grad = np.zeros(pos.shape)
        grad[0,1] = float(pos[0,1] < MIN_Y)
        return grad
    
    constraint_manager.push(
        f"tableSurface({ball.number})",
        [ball.node.id],
        value,
        grad,
    )

def register_table_short_side_constraint(ball, constraint_manager):
    MAX_X = HALF_SHORT_SIDE - BALL_RADIUS
    MIN_X = - MAX_X

    def value(pos: np.ndarray) -> np.floating:
        return np.minimum(pos[0,0] - MIN_X, 0) + np.minimum(MAX_X - pos[0,0], 0)
    
    def grad(pos: np.ndarray) -> np.ndarray:
        grad = np.zeros(pos.shape)
        grad[0,0] = float(pos[0,0] < MIN_X) - float(pos[0,0] > MAX_X)
        return grad
    
    constraint_manager.push(
        f"tableShortSide({ball.number})",
        [ball.node.id],
        value,
        grad,
    )

def register_table_long_side_constraint(ball, constraint_manager):
    MAX_Z = HALF_LONG_SIDE - BALL_RADIUS
    MIN_Z = - MAX_Z

    def value(pos: np.ndarray) -> np.floating:
        return np.minimum(pos[0,2] - MIN_Z, 0) + np.minimum(MAX_Z - pos[0,2], 0)
    
    def grad(pos: np.ndarray) -> np.ndarray:
        grad = np.zeros(pos.shape)
        grad[0,2] = float(pos[0,2] < MIN_Z) - float(pos[0,2] > MAX_Z)
        return grad
    
    constraint_manager.push(
        f"tableLongSide({ball.number})",
        [ball.node.id],
        value,
        grad,
    )