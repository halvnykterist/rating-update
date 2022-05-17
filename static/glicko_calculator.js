"use strict";

const q = 0.00575646273;

const g = (rating_deviation) => {
    return 1.0 / Math.sqrt(1.0 + (3.0 * Math.pow(q, 2.0) * Math.pow(rating_deviation, 2.0)) / Math.pow(Math.PI, 2.0));
}

const e = (own_rating, other_rating) => {
    return 1.0 / (1.0 + Math.pow(10.0, g(other_rating.deviation) * (own_rating.value - other_rating.value) / -400.0));
}

const d2 = (own_rating, other_rating) => {
    return 1.0 / (Math.pow(q, 2.0) 
        * Math.pow(g(other_rating.deviation), 2.0)
        * e(own_rating, other_rating)
        * (1.0 - e(own_rating, other_rating)));
}

const new_rating_value = (own_rating, other_rating, outcome) => {
    return (q / (1.0 / Math.pow(other_rating.deviation, 2.0) + (1.0 / d2(own_rating, other_rating)))) * g(other_rating.deviation) * (outcome - e(own_rating, other_rating));
}

const new_rating_deviation = (own_rating, other_rating) => {
    return Math.sqrt(1.0 / 
        ((1.0 / Math.pow(other_rating.deviation, 2.0)) + (1.0 / d2(own_rating, other_rating))));
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

const update = () => {
    const own_rating = {
        value: document.getElementById("own_rating").valueAsNumber,
        deviation: document.getElementById("own_deviation").valueAsNumber,
    };

    const other_rating = {
        value: document.getElementById("opp_rating").valueAsNumber,
        deviation: document.getElementById("opp_deviation").valueAsNumber,
    };

    console.log(g(other_rating.deviation));
    console.log(e(own_rating, other_rating));
    console.log(d2(own_rating, other_rating));

    document.getElementById("expected_outcome").innerHTML = Math.round(expected_outcome(own_rating, other_rating) * 100.0) + "%";

    const own_win_delta = new_rating_value(own_rating, other_rating, 1.0) - own_rating.value;
    const own_lose_delta = new_rating_value(own_rating, other_rating, 0.0) - own_rating.value;
    const own_new_deviation = new_rating_deviation(own_rating, other_rating);

    const other_win_delta = new_rating_value(other_rating, own_rating, 1.0) - other_rating.value;
    const other_lose_delta = new_rating_value(other_rating, own_rating, 0.0) - other_rating.value;
    const other_new_deviation = new_rating_deviation(other_rating, own_rating);

    document.getElementById("own_win_delta").innerHTML = Math.round(own_win_delta * 10.0) / 10.0;
    document.getElementById("opp_win_delta").innerHTML = Math.round(other_win_delta * 10.0) / 10.0;
}
