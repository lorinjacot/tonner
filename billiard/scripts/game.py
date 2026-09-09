from typing import Dict
import billiard as b
from numpy.typing import NDArray

import numpy as np

SOLID_COLORS = [
    b.BallColor.SolidYellow,
    b.BallColor.SolidBlue,
    b.BallColor.SolidRed,
    b.BallColor.SolidPurple,
    b.BallColor.SolidOrange,
    b.BallColor.SolidGreen,
    b.BallColor.SolidMaroon,
]

STRIPE_COLORS = [
    b.BallColor.YellowStripe,
    b.BallColor.BlueStripe,
    b.BallColor.RedStripe,
    b.BallColor.PurpleStripe,
    b.BallColor.OrangeStripe,
    b.BallColor.GreenStripe,
    b.BallColor.MaroonStripe,
]

BASE_POS = np.array([0.0, 0.025, 0.65])
BALL_DISTANCE = 0.05


class Game:
    balls: Dict[b.BallColor, b.Ball]
    white: b.Ball
    black: b.Ball
    solid: Dict[b.BallColor, b.Ball]
    stripe: Dict[b.BallColor, b.Ball]

    def __init__(self, balls: Dict[b.BallColor, b.Ball]) -> None:
        self.white = balls[b.BallColor.White]
        self.black = balls[b.BallColor.Black]
        self.solid = {color: balls[color] for color in SOLID_COLORS}
        self.stripe = {color: balls[color] for color in STRIPE_COLORS}
        self.balls = balls.copy()

        self.reset_balls()

    def reset_balls(self) -> None:
        row0 = b.BallColor.SolidYellow
        row1 = (b.BallColor.GreenStripe, b.BallColor.RedStripe)
        row2 = (b.BallColor.SolidOrange, b.BallColor.Black, b.BallColor.SolidMaroon)
        row3 = (
            b.BallColor.MaroonStripe,
            b.BallColor.SolidBlue,
            b.BallColor.PurpleStripe,
            b.BallColor.YellowStripe,
        )
        row4 = (
            b.BallColor.SolidPurple,
            b.BallColor.BlueStripe,
            b.BallColor.SolidRed,
            b.BallColor.OrangeStripe,
            b.BallColor.SolidGreen,
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
