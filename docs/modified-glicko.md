It is my firm belief that matchmaking based on skill is one of the most important things that fighting games are currently missing. Much is made about fighting games being 'difficult', but fundamentally these are arcade games that you're meant to be able to just pick up and play; and you absolutely can, provided you find a similarly skilled opponent to play with.

1. [Skill systems, what are they?](#skill-systems)
    1. [Skill](#skill)
    2. [Skill uncertainty](#skill-uncertainty)
2. [How rating update works](#how-rating-update-works)
    1. [Step 1: Decay deviation](#step-1%3A-decay-deviation)
    2. [Step 2: Update ratings](#step-2%3A-update-ratings)
    3. [Step 3: Update deviation](#step-3%3A-update-deviation)
    4. [Calculating win probabilities](#calculating-win-probabilities)



## Skill systems
I'm not at all the most qualified to talk about this, merely very enthusiastic. For better overviews I highly recommend watching the GDC talks by Josh Menke, who's designed skill and rating systems for a ton of different games, for example [Skill, Matchmaking, and Ranking Systems Design](https://youtu.be/-pglxege-gU).


### Skill
Most skill systems use some variation of the [Bradley-Terry model](https://en.wikipedia.org/wiki/Bradley%E2%80%93Terry_model) to predict player interactions (say, Alice should beat Bob 60% of the time) and then updating the skills according to game results. If Alice beats Bob more than 60% of the time, her skill rating will increase, if she doesn't, it'll drop.

On the Elo scale, you can look at the difference in rating to determine your probability of winning:

| Rating difference | Win probability|
|:-----------------:|:--------------:|
|       0           |       50%      |
|       50          |       57%      |
|      100          |       64%      |
|      150          |       70%      |
|      200          |       76%      |
|      250          |       81%      |
|      300          |       85%      |
|      400          |       91%      |
|      500          |       96%      |
|      600          |       98%      |

I'd say anything under 150 difference in rating is acceptable since then you at least have a 50% probability of picking up a game in a FT2, even if you're likely to lose overall.


### Skill uncertainty
On ratingupdate, your skill rating is shown using two numbers in the format `X ±Y`, where `X` is your actual rating and `Y` represents how certain the system in in that rating - specifically, it's 95% certain that you're somewhere between `X - Y` and `X + Y`. 

When you first start playing your uncertainty (or _deviation_), is set to `700`. This is because the system knows nothing at all about you yet, and it could turn out that you're either completely new to the game and have a skill level around `1500 - 700  = 800` or a seasoned veteran with a skill level around `1500 + 700 = 2200`. 

As you play matches and the system learns more about you, how certain the system is in your rating increases.
How certain the system is in your rating also affects how much your rating changes: If the system knows nothing at all about you yet (high uncertainty) your rating will move more, but if you have low uncertainty then your rating won't move much even if you lose or win quite a few games in a row, since that kind of thing is to be expected.

Additionally, how uncertain your opponent's rating is  matters - losing against someone with high uncertainty doesn't say much, since you can't really know if you're better or worse than a given rating when we're not even sure what rating you just fought against was.


## How rating update works
Rating-update used to run on Glicko-2, but this new version uses a modified version the Glicko-1 algorithm that avoids the problem of having rating periods. This means the ratings get updated continuously, after each game, so it's suitable for use in matchmaking.

##### Step 1: Decay deviation
The algorithm needs to increase deviation over some time in order to have some "play" and to model the fact  that if we haven't seen someone play in a month, we really don't know as much about them as if they just played a bunch of sets.

Vanilla Glicko handles decay in connection with _rating periods_, but since we're not using those we instead decay based on how many "virtual rating periods" have passed since the last time we decayed the deviation. You could probably be smarter and do this without a loop, but this was clearest as a starting point.

`C` here is a constant  you can change to tune how quickly player's ratings decay. In principle, you should experiment on your data to see what value gives you the best predictive power, but a simpler starting point is just solving `INITIAL_DEVIATION = sqrt(AVERAGE_DEVIATION^2 * NUM_PERIODS * C^2)` for`C` in order to figure out a value where it'll take `NUM_PERIODS` to get from `AVERAGE_DEVIATION` back to the `INITIAL_DEVIATION` that every new player has.

`INITIAL_DEVIATION` is set to `350` on the original Glicko scale, and that's what rating-update uses too, although deviations are doubled for presentations to achieve 95% certainty.


```rust
//I apologize for using Rust as syntax for examples, but that's what the
//codebase is written in and I'll try to keep it simple.

//Call this for both players' ratings, using the time since the last time we
//updated this player's rating as time_elapsed.
fn decay_deviation(rating: &mut Rating, time_elapsed: i64) {
    //1. Figure out how many cycles of decay we need to use
    let decay_count = time_elapsed / RATING_PERIOD_LENGTH;
    //2. Loop that many times, decaying the rating each time.
    for _ in 0..decay_count {
        //3. Update according to Glicko's formula.
        rating.deviation = f64::sqrt(f64::powf(rating.deviation, 2.0) + f64::powf(C, 2.0)); 
        //4. Clamp the value to prevent it reaching higher than the initial deviation.
        rating.deviation = f64::min(rating.deviation, INITIAL_DEVIATION);
    }
}
```

##### Step 2: Update ratings
As mentioned, the original algorithm handles updates in terms of rating periods, where all players are updated simultaneously. This has some upsides, but is unsuitable for realtime applications or anything like MMR where we'd want to be able to quickly find a player's skill and match them to good opponents.

This also means we're only dealing with a game per "rating period", so we can simplify the formulas a bit.

First of all, we need to determine some intermediate values that'll help us calculate new ratings and deviations.


`q` Is really a constant that what use to transform rating scale:

```rust
//This is really just 0.00575646273
const Q: f64 = f64::ln(10.0) / 400.0;
```

`g` is a number that represents how a rating's certainty affects outcomes:

```rust
//Hairy looking, I know
fn calc_g(rating_deviation: f64) -> f64 {
    1.0 / 
        f64::sqrt(1.0 + 
            (3.0 * f64::powf(Q, 2.0) * f64::powf(rating_deviation, 2.0))
            / f64::powf(PI, 2.0))
}
```

This looks very hairy but if we run a through deviations through it we can roughly see what's going. As rating deviation increases, we're more and more certain ratings and weight things accordingly.

|  `deviation` | `g(deviation)` |
|:------------:|:-------------:|
|      350     |     0.67      |
|      300     |     0.72      |
|      250     |     0.78      |
|      200     |     0.84      |
|      150     |     0.90      |
|      100     |     0.95      |
|       50     |     0.99      |

`e` represents the expected value of a match, ignoring one side's deviation for now. This is the [logistic curve](https://en.wikipedia.org/wiki/Logistic_function) used in the Elo system, but using `g` to weight the outcomes. Someone being rated 200 higher than you doesn't necessarily mean much if their deviation is through the roof, at best the algorithm can shrug and say "yeah I guess maybe he'll win but I'm not that sure"

```rust
fn calc_e(own_rating: Rating, other_rating: Rating) -> f64 {
    let rating_difference = own_rating.rating - other_rating.rating;
    let g = calc_g(other_rating.deviation);

    1.0 / (1.0 + f64::powf(10.0, g * rating_difference / -400.0))
}
```

`d^2` is used to help determine how quickly your rating should move. For example, we get a lot more information from close matches (where the expected outcome is near 50%) than we do from blowouts. If you're expected to beat someone between 80% and 95% of time, winning doesn't really help us guess if it's closer to 90% or 95%.

```rust
fn calc_d2(own_rating: Rating, other_rating: Rating) -> f64 {
    let g = calc_g(other_rating.deviation);
    let e = calc_e(own_rating, other_rating);

    1.0 / (f64::powf(Q, 2.0) * f64::powf(g, 2.0) * e * (1.0 - e))
}
```

Now we can finally determine the new rating for a player. `outcome` is `1` for a win or `0` for a loss. If we had ties those would be `0.5`, but those don't apply to fighting games very often.

```rust
fn calc_new_rating(own_rating: Rating, other_rating: Rating, outcome: f64) -> f64 {
    let g = calc_g(other_rating.deviation);
    let e = calc_e(own_rating, other_rating);
    let d_2 = calc_d2(own_rating, other_rating);
    
    (Q / (1.0 / f64::powf(own_rating.deviation, 2.0)) + (1.0 / d_2)) * g * (outcome - e)
}
```

##### Step 3: Update deviation
Now that we've seen the player play a game we can update the deviation to represent our increased certainty in whatever rating they have.
```rust
fn calc_new_deviation(own_rating: Rating, other_rating: Rating) -> f64 {
    let d_2 = calc_d2(own_rating, other_rating);
    f64::sqrt(1.0 / ((1.0 / f64::powf(own_rating.deviation, 2.0)) + (1.0 / d_2)))
}
```

##### Calculating win probabilities
To calculate the expected outcome of match taking both deviations into account, you can use the following formula:
```rust
fn calc_expected_outcome(own_rating: Rating, other_rating: Rating) -> f64 {
    let rating_difference = own_rating.rating - other_rating.rating;
    //This is the bit that's different from calc_e!
    let g = calc_g(f64::sqrt(
        f64::powf(own_rating.deviation, 2.0) + f64::powf(other_rating.deviation, 2.0),
    ));

    1.0 / (1.0 + f64::powf(10.0, g * rating_difference / -400.0))
}
```
