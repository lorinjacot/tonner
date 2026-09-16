from enum import Enum

from numpy.typing import NDArray

class Ball:
    color: BallColor
    radius: float
    out: bool
    number: int
    position: NDArray
    velocity: NDArray

class BallColor(Enum):
    White = 0
    SolidYellow = 1
    SolidBlue = 2
    SolidRed = 3
    SolidPurple = 4
    SolidOrange = 5
    SolidGreen = 6
    SolidMaroon = 7
    Black = 8
    YellowStripe = 9
    BlueStripe = 10
    RedStripe = 11
    PurpleStripe = 12
    OrangeStripe = 13
    GreenStripe = 14
    MaroonStripe = 15

class UiState:
    class Startup(UiState):
        pass

    class MainMenu(UiState):
        pass

    class InGame(UiState):
        game_state: GameState

    class GameOver(UiState):
        winner: Player

class GameState:
    class Playing(GameState):
        turn: Player

    class Watching(GameState):
        last: Player

class Player:
    class Solid(Player):
        pass

    class Stripe(Player):
        pass
