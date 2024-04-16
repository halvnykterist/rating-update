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

```bash
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

<details>
 <summary><code>GET</code> <code><b>/api/weekly_games?length=#</b></code> (gets stats about all player activity over the last # weeks)</summary>

##### Parameters

> | name              |  type     | data type      | description                         | default    |
> |-------------------|-----------|----------------|-------------------------------------|------------|
> | `length`          |  optional | int ($i64)     | Amount of weeks to be queried        | 8         |

##### Code : `200 OK`

The values are returned as 3 arrays.  
Values belong together when they are in the same position inside their corresponding arrays.  
The first array contains the dates of the weeks.  
The second array contains the amount of games played every week.  
The third array contains how many players were active every week.

```json
[
  [
    "2024-04-09"
  ],
  [
    100
  ],
  [
    20
  ]
]
```
`Example`: In the week of 9. April 2024 there were 100 games played by 20 distinct players. 

##### Example cURL

```bash
curl -X GET -H "Content-Type: application/json" http://ratingupdate.info/api/weekly_games?length=2
```
</details>

<details>
 <summary><code>GET</code> <code><b>/api/daily_character_games?length=#</b></code> (gets stats about all player activity over the last # days by character)</summary>

##### Parameters

> | name              |  type     | data type      | description                         | default    |
> |-------------------|-----------|----------------|-------------------------------------|------------|
> | `length`          |  optional | int ($i64)     | Amount of days to be queried        | 60         |

##### Code : `200 OK`

The values are returned as 3 arrays.  
Values belong together when they are in the same position inside their corresponding arrays.  
The first array contains the dates of the days.  
The second array contains the characters. 
The third array contains how many games were played with every character every day.

```json
[
  [
    "2024-04-11",
    "2024-04-12",
  ],
  [
    "SO",
    "KY",
    "MA",
    ...
  ],
  [
    [15, 8],
    [7, 9],
    [11, 9],
    ...
  ]
]
```
`Example`: On the 11. April 2024 there were 15 games with Sol played. 

##### Example cURL

```bash
curl -X GET -H "Content-Type: application/json" http://ratingupdate.info/api/daily_character_games?length=2
```
</details>

<details>
 <summary><code>GET</code> <code><b>/api/active_players</b></code> (gets the amount of daily active players over the last two weeks)</summary>

##### Parameters

> None

##### Code : `200 OK`

```json
[
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0
]
```

##### Example cURL

```bash
curl -X GET -H "Content-Type: application/json" http://ratingupdate.info/api/active_players
```
</details>

#### Rating Endpoints

<details>
 <summary><code>GET</code> <code><b>/api/top/all</b></code> (gets the top 100 players)</summary>

##### Parameters

> None

##### Code : `200 OK`

```json
[
  {
    "pos": 1,
    "id": "2EC405FD6B5A8C9",
    "platform": "PC",
    "character": "Goldlewis",
    "character_short": "GO",
    "name": "UA Rang13",
    "game_count": 6808,
    "rating_value": 2361,
    "rating_deviation": 98,
    "vip_status": null,
    "cheater_status": null,
    "hidden_status": null
  },
  {
    "pos": 2,
    "id": "2EC3DC4435865B9",
    "platform": "PC",
    "character": "Sol",
    "character_short": "SO",
    "name": "UMISHO",
    "game_count": 4109,
    "rating_value": 2350,
    "rating_deviation": 125,
    "vip_status": null,
    "cheater_status": null,
    "hidden_status": null
  },
  ...
]
``` 

##### Example cURL

```bash
curl -X GET -H "Content-Type: application/json" http://ratingupdate.info/api/top/all
```
</details>

<details>
 <summary><code>GET</code> <code><b>/api/top/:char_id</b></code> (gets the top 100 players for a given character)</summary>

##### Parameters

> | name              |  type     | data type      | description                         | default    |
> |-------------------|-----------|----------------|-------------------------------------|------------|
> | `char_id`          |  required | int ($i64)     | Id of character (Sol=0)        | -         |

##### Code : `200 OK`

```json
[
  {
    "pos": 1,
    "id": "310D591D4B0853B",
    "platform": "PC",
    "character": "Ky",
    "character_short": "KY",
    "name": "DM El Maza",
    "game_count": 3876,
    "rating_value": 2241,
    "rating_deviation": 101,
    "vip_status": null,
    "cheater_status": null,
    "hidden_status": null
  },
  {
    "pos": 2,
    "id": "2EDB72A74608783",
    "platform": "PC",
    "character": "Ky",
    "character_short": "KY",
    "name": "Useless Miwa",
    "game_count": 944,
    "rating_value": 2227,
    "rating_deviation": 113,
    "vip_status": null,
    "cheater_status": null,
    "hidden_status": null
  },
  ...
]
``` 

##### Example cURL

```bash
curl -X GET -H "Content-Type: application/json" http://ratingupdate.info/api/top/1
```
</details>

<details>
 <summary><code>GET</code> <code><b>/api/player_rating/:player</b></code> (gets the rating of a player)</summary>

##### Parameters

> | name              |  type     | data type      | description                         | default    |
> |-------------------|-----------|----------------|-------------------------------------|------------|
> | `player`          |  required | hex     | Hexadecimal representation of the player id        | -         |


##### Code : `200 OK`

The result is a array where every item corresponds to a character.

```json
[
  {
    "value": 1500,
    "deviation": 350
  },
  {
    "value": 1341.3941831910975,
    "deviation": 67.10095580454131
  },
  {
    "value": 1500,
    "deviation": 350
  },
  ...
]
``` 

##### Example cURL

```bash
curl -X GET -H "Content-Type: application/json" http://ratingupdate.info/api/player_rating/31081E2F439665D
```
</details>

<details>
 <summary><code>GET</code> <code><b>/api/player_rating/:player/:character_short</b></code> (gets the rating of a player with a certain character)</summary>

##### Parameters

> | name              |  type     | data type      | description                         | default    |
> |-------------------|-----------|----------------|-------------------------------------|------------|
> | `player`          |  required | hex     | Hexadecimal representation of the player id        | -         |
> | `character_short`          |  required | str(2)     | Two letter shorthand for a character        | -         |


##### Code : `200 OK`

```json
{
  "value": 1341.3941831910975,
  "deviation": 67.10095580454131
}
``` 

##### Example cURL

```bash
curl -X GET -H "Content-Type: application/json" http://ratingupdate.info/api/player_rating/31081E2F439665D/PO
```
</details>

<details>
 <summary><code>GET</code> <code><b>/api/accuracy/:player/:character_short</b></code> (TODO: NOT SURE YET)</summary>

##### Parameters

> | name              |  type     | data type      | description                         | default    |
> |-------------------|-----------|----------------|-------------------------------------|------------|
> | `player`          |  required | hex     | Hexadecimal representation of the player id        | -         |
> | `character_short`          |  required | str(2)     | Two letter shorthand for a character        | -         |


##### Code : `200 OK`

The reponse is an array containing 11 values, each corresponding to the winrate? of the player against with a certain skillgap.

```json
[
  null,
  null,
  0.3333333333333333,
  0.14285714285714285,
  0.375,
  0.5238095238095238,
  0.8,
  0.7142857142857143,
  0.75,
  null,
  null
]
``` 

##### Example cURL

```bash
curl -X GET -H "Content-Type: application/json" http://ratingupdate.info/api/accuracy/31081E2F439665D/PO
```
</details>

#### Search Endpoints

<details>
 <summary><code>GET</code> <code><b>/api/player_lookup?name=#</b></code> (gets rating information about players with a specific name)</summary>

##### Parameters

> | name              |  type     | data type      | description                         | default    |
> |-------------------|-----------|----------------|-------------------------------------|------------|
> | `name`          |  required | str     | name of the player        | -         |

Note: this search is exact, `name=Michioc` will not return players named `Michiocre`.

##### Code : `200 OK`

```json
[
  {
    "id": "31081E2F439665D",
    "name": "Michiocre",
    "characters": [
      {
        "shortname": "PO",
        "rating": 1341,
        "deviation": 134,
        "game_count": 765
      }
    ]
  }
]
``` 

##### Example cURL

```bash
curl -X GET -H "Content-Type: application/json" http://ratingupdate.info/api/player_lookup?name=Michiocre
```
</details>

<details>
 <summary><code>GET</code> <code><b>/api/search?name=#</b></code> (gets information about players with given name)</summary>

##### Parameters

> | name              |  type     | data type      | description                         | default    |
> |-------------------|-----------|----------------|-------------------------------------|------------|
> | `name`          |  required | str     | name of the player        | -         |

Note: this search is <b>NOT</b> exact, `name=Michioc` will also return players named `Michiocre`.

##### Code : `200 OK`

The reponse is an array containing one entry per played character of every player found with the given name.

```json
[
  {
    "name": "Michiocre",
    "platform": "PC",
    "vip_status": null,
    "cheater_status": null,
    "hidden_status": null,
    "id": "31081E2F439665D",
    "character": "Potemkin",
    "character_short": "PO",
    "rating_value": 1341,
    "rating_deviation": 134,
    "game_count": 765
  }
]
``` 

##### Example cURL

```bash
curl -X GET -H "Content-Type: application/json" http://ratingupdate.info/api/search?name=Michiocre
```
</details>

<details>
 <summary><code>GET</code> <code><b>/api/search_exact?name=#</b></code> (gets information about players with exact given name)</summary>

##### Parameters

> | name              |  type     | data type      | description                         | default    |
> |-------------------|-----------|----------------|-------------------------------------|------------|
> | `name`          |  required | str     | name of the player        | -         |

Note: this search is exact, `name=Michioc` will not return players named `Michiocre`.

##### Code : `200 OK`

The reponse is an array containing one entry per played character of every player found with the exact given name.

```json
[
  {
    "name": "Michiocre",
    "platform": "PC",
    "vip_status": null,
    "cheater_status": null,
    "hidden_status": null,
    "id": "31081E2F439665D",
    "character": "Potemkin",
    "character_short": "PO",
    "rating_value": 1341,
    "rating_deviation": 134,
    "game_count": 765
  }
]
``` 

##### Example cURL

```bash
curl -X GET -H "Content-Type: application/json" http://ratingupdate.info/api/search_exact?name=Michiocre
```
</details>