from typing import Dict
from .billiard import BallColor, Ball
from numpy.typing import NDArray

import numpy as np

SOLID_COLORS = [
    BallColor.SolidYellow,
    BallColor.SolidBlue,
    BallColor.SolidRed,
    BallColor.SolidPurple,
    BallColor.SolidOrange,
    BallColor.SolidGreen,
    BallColor.SolidMaroon,
]

STRIPE_COLORS = [
    BallColor.YellowStripe,
    BallColor.BlueStripe,
    BallColor.RedStripe,
    BallColor.PurpleStripe,
    BallColor.OrangeStripe,
    BallColor.GreenStripe,
    BallColor.MaroonStripe,
]

BASE_POS = np.array([0.0, 0.025, 0.65])
BALL_DISTANCE = 0.05


class Game:
    balls: Dict[BallColor, Ball]
    white: Ball
    black: Ball
    solid: Dict[BallColor, Ball]
    stripe: Dict[BallColor, Ball]

    def __init__(self, balls: Dict[BallColor, Ball]) -> None:
        self.white = balls[BallColor.White]
        self.black = balls[BallColor.Black]
        self.solid = {color: balls[color] for color in SOLID_COLORS}
        self.stripe = {color: balls[color] for color in STRIPE_COLORS}
        self.balls = balls.copy()

        self.reset_balls()

    def reset_balls(self) -> None:
        row0 = BallColor.SolidYellow
        row1 = (BallColor.GreenStripe, BallColor.RedStripe)
        row2 = (BallColor.SolidOrange, BallColor.Black, BallColor.SolidMaroon)
        row3 = (
            BallColor.MaroonStripe,
            BallColor.SolidBlue,
            BallColor.PurpleStripe,
            BallColor.YellowStripe,
        )
        row4 = (
            BallColor.SolidPurple,
            BallColor.BlueStripe,
            BallColor.SolidRed,
            BallColor.OrangeStripe,
            BallColor.SolidGreen,
        )

        dz = np.sqrt(3) / 2 * BALL_DISTANCE
        dx = BALL_DISTANCE

        self.white.position = np.array([0.0, 0.025, -0.8])

        self.balls[row0].position = np.array([0, 0, 0]) + BASE_POS

        self.balls[row1[0]].position = np.array([-dx / 2, 0, dz]) + BASE_POS
        self.balls[row1[1]].position = np.array([dx / 2, 0, dz]) + BASE_POS

        self.balls[row2[0]].position = np.array([-dx, 0, 2 * dz]) + BASE_POS
        self.balls[row2[1]].position = np.array([0, 0, 2 * dz]) + BASE_POS
        self.balls[row2[2]].position = np.array([dx, 0, 2 * dz]) + BASE_POS

        self.balls[row3[0]].position = np.array([-3 * dx / 2, 0, 3 * dz]) + BASE_POS
        self.balls[row3[1]].position = np.array([-dx / 2, 0, 3 * dz]) + BASE_POS
        self.balls[row3[2]].position = np.array([dx / 2, 0, 3 * dz]) + BASE_POS
        self.balls[row3[3]].position = np.array([3 * dx / 2, 0, 3 * dz]) + BASE_POS

        self.balls[row4[0]].position = np.array([-2 * dx, 0, 4 * dz]) + BASE_POS
        self.balls[row4[1]].position = np.array([-dx, 0, 4 * dz]) + BASE_POS
        self.balls[row4[2]].position = np.array([0, 0, 4 * dz]) + BASE_POS
        self.balls[row4[3]].position = np.array([dx, 0, 4 * dz]) + BASE_POS
        self.balls[row4[4]].position = np.array([2 * dx, 0, 4 * dz]) + BASE_POS

        for ball in self.balls.values():
            ball.velocity = np.array([0.0, 0.0, 0.0])
