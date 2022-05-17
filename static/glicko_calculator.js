"use strict";

const q = 0.00575646273;

const g = (rating_deviation) => {
    return 1.0 / Math.sqrt(1.0 + (3.0 * Math.pow(q, 2.0) * Math.pow(rating_deviation, 2.0)) / Math.pow(Math.PI, 2.0));
}

const e = (own_rating, other_rating) => {
    return 1.0 / (1.0 + Math.pow(10.0, g(other_rating.deviation) * (own_rating.value - other_rating.value) / -400.0));
}

const d2 = (own_rating, other_rating) => {
    return 1.0 / (
        Math.pow(q, 2.0) 
        * Math.pow(g(other_rating.deviation), 2.0)
        * e(own_rating, other_rating)
        * (1.0 - e(own_rating, other_rating)));
}

const new_rating_value = (own_rating, other_rating, outcome) => {
    return own_rating.value + (q / (1.0 / Math.pow(own_rating.deviation, 2.0) + (1.0 / d2(own_rating, other_rating)))) * g(other_rating.deviation) * (outcome - e(own_rating, other_rating));
}

const new_rating_deviation = (own_rating, other_rating) => {
    return Math.max(25.0, Math.sqrt(1.0 / ((1.0 / Math.pow(own_rating.deviation, 2.0)) + (1.0 / d2(own_rating, other_rating)))));
}

const expected_outcome = (own_rating, other_rating) => {
    return 1.0 / (1.0 + Math.pow(10.0,
        g(Math.sqrt(Math.pow(own_rating.deviation, 2.0) + Math.pow(other_rating.deviation, 2.0)))
        * (own_rating.value - other_rating.value) / -400));
}

const colorize = (rating_delta) => {
    if (rating_delta >= 2.0) {
        return "rating-up";
    } else if (rating_delta >= 0.0) {
        return "rating-barely-up";
    } else if (rating_delta > -2.0) {
        return "rating-barely-down";
    } else {
        return "rating-down";
    }
}

const set_win_probability = (x) => 3 * Math.pow(x, 2) - 2 * Math.pow(x, 3);

const update = () => {
    const own_rating = {
        value: document.getElementById("own_rating").valueAsNumber,
        deviation: document.getElementById("own_deviation").valueAsNumber * 0.5,
    };

    const other_rating = {
        value: document.getElementById("opp_rating").valueAsNumber,
        deviation: document.getElementById("opp_deviation").valueAsNumber * 0.5,
    };

    const win_chance = expected_outcome(own_rating, other_rating);
    const set_win_chance = set_win_probability(win_chance);

    document.getElementById("expected_outcome").innerHTML = (win_chance * 100.0).toFixed(0) + "%";

    const own_win_delta = new_rating_value(own_rating, other_rating, 1.0) - own_rating.value;
    const own_loss_delta = new_rating_value(own_rating, other_rating, 0.0) - own_rating.value;

    const other_win_delta = new_rating_value(other_rating, own_rating, 1.0) - other_rating.value;
    const other_loss_delta = new_rating_value(other_rating, own_rating, 0.0) - other_rating.value;

    const f = new Intl.NumberFormat('en-IN', {
        signDisplay: "always",
        minimumFractionDigits: 1,
        maximumFractionDigits: 1,
    });


    document.getElementById("own_win_delta").innerHTML = f.format(own_win_delta);
    document.getElementById("own_win_delta").className = colorize(own_win_delta);
    document.getElementById("own_loss_delta").innerHTML = f.format(own_loss_delta);
    document.getElementById("own_loss_delta").className = colorize(own_loss_delta);
    document.getElementById("opp_win_delta").innerHTML = f.format(other_win_delta);
    document.getElementById("opp_win_delta").className = colorize(other_win_delta);
    document.getElementById("opp_loss_delta").innerHTML = f.format(other_loss_delta);
    document.getElementById("opp_loss_delta").className = colorize(other_loss_delta);

    const own_new_deviation = new_rating_deviation(own_rating, other_rating);
    const other_new_deviation = new_rating_deviation(other_rating, own_rating);

    document.getElementById("own_new_deviation").innerHTML = Math.round(own_new_deviation * 2);
    document.getElementById("opp_new_deviation").innerHTML = Math.round(other_new_deviation * 2);
}
