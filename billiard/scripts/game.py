from typing import Dict
import billiard
from numpy.typing import NDArray

import numpy as np

SOLID_COLORS = [
    billiard.BallColor.SolidYellow,
    billiard.BallColor.SolidBlue,
    billiard.BallColor.SolidRed,
    billiard.BallColor.SolidPurple,
    billiard.BallColor.SolidOrange,
    billiard.BallColor.SolidGreen,
    billiard.BallColor.SolidMaroon,
]

STRIPE_COLORS = [
    billiard.BallColor.YellowStripe,
    billiard.BallColor.BlueStripe,
    billiard.BallColor.RedStripe,
    billiard.BallColor.PurpleStripe,
    billiard.BallColor.OrangeStripe,
    billiard.BallColor.GreenStripe,
    billiard.BallColor.MaroonStripe,
]

BASE_POS = np.array([0.0, 0.025, 0.65])
BALL_DISTANCE = 0.05


class Game:
    balls: Dict[billiard.BallColor, Ball]
    white: billiard.Ball
    black: billiard.Ball
    solid: Dict[billiard.BallColor, Ball]
    stripe: Dict[billiard.BallColor, Ball]

    def __init__(self, balls: Dict[billiard.BallColor, billiard.Ball]) -> None:
        self.white = balls[billiard.BallColor.White]
        self.black = balls[billiard.BallColor.Black]
        self.solid = {color: balls[color] for color in SOLID_COLORS}
        self.stripe = {color: balls[color] for color in STRIPE_COLORS}
        self.balls = balls.copy()

        self.reset_balls()

    def reset_balls(self) -> None:
        row0 = billiard.BallColor.SolidYellow
        row1 = (billiard.BallColor.GreenStripe, billiard.BallColor.RedStripe)
        row2 = (billiard.BallColor.SolidOrange, billiard.BallColor.Black, billiard.BallColor.SolidMaroon)
        row3 = (
            billiard.BallColor.MaroonStripe,
            billiard.BallColor.SolidBlue,
            billiard.BallColor.PurpleStripe,
            billiard.BallColor.YellowStripe,
        )
        row4 = (
            billiard.BallColor.SolidPurple,
            billiard.BallColor.BlueStripe,
            billiard.BallColor.SolidRed,
            billiard.BallColor.OrangeStripe,
            billiard.BallColor.SolidGreen,
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
