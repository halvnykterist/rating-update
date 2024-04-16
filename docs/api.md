## Rating-Update API

#### General Endpoints

<details>
 <summary><code>GET</code> <code><b>/api/stats</b></code> (gets stats about all player activity)</summary>

##### Parameters

> None

##### Code : `200 OK`

```json
{
    "game_count": 21334901,
    "player_count": 358534,
    "activity_7d": {
        "players": 0,
        "games": 0,
        "over_1700": 0,
        "over_1900": 0,
        "over_2100": 0,
        "sub_1300": 0,
        "sub_1100": 0,
        "sub_900": 0
    },
    "activity_24h": {
        "players": 0,
        "games": 0,
        "over_1700": 0,
        "over_1900": 0,
        "over_2100": 0,
        "sub_1300": 0,
        "sub_1100": 0,
        "sub_900": 0
    },
    "activity_1h": {
        "players": 0,
        "games": 0,
        "over_1700": 0,
        "over_1900": 0,
        "over_2100": 0,
        "sub_1300": 0,
        "sub_1100": 0,
        "sub_900": 0
    }
}
```

##### Example cURL

```javascript
curl -X GET -H "Content-Type: application/json" http://ratingupdate.info/api/stats
```
</details>

<details>
 <summary><code>GET</code> <code><b>/api/daily_games?length=#</b></code> (gets stats about all player activity over the last # days)</summary>

##### Parameters

> | name              |  type     | data type      | description                         | default    |
> |-------------------|-----------|----------------|-------------------------------------|------------|
> | `length`          |  optional | int ($i64)     | Amount of days to be queried        | 60         |

##### Code : `200 OK`

The values are returned as 3 arrays.  
Values belong together when they are in the same position inside their corresponding arrays.  
The first array contains the dates of the days.  
The second array contains the amount of games played every day.  
The third array contains how many players were active every day.

```json
[
    [
        "2024-04-11",
        "2024-04-12",
        "2024-04-13",
        "2024-04-14",
        "2024-04-15"
    ],
    [
        10,
        5,
        6,
        2,
        1
    ],
    [
        4,
        3,
        3,
        2,
        2
    ]
]
```
`Example`: On the 11. April 2024 there were 10 games played by 4 distinct players. 

##### Example cURL

```bash
curl -X GET -H "Content-Type: application/json" http://ratingupdate.info/api/daily_games?length=5
```
</details>
