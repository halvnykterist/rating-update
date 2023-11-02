import os
from time import time
from requests import get, post
from socket import inet_aton
from base64 import urlsafe_b64encode
from binascii import crc32, hexlify, unhexlify
from steam.client import SteamClient
from steam.core.msg import MsgProto
from steam.enums.emsg import EMsg
from steam.utils.proto import proto_fill_from_dict
from Crypto.Cipher import AES
from Crypto.Random import get_random_bytes
from msgpack import packb, unpackb
from struct import pack

def main():
    user = os.environ['USER']
    password = os.environ['PASSWORD']
    token = login(user, password)
    replays = get_replays(token)
    print(replays)


game_tokens = []

def on_game_tokens(msg):
    global game_tokens
    print(msg)
    game_tokens.extend(msg.body.tokens)


def create_auth_ticket(token, session_time):
    # dumb shit
    session_size = 24
    public_ip = get('https://checkip.amazonaws.com').text.strip()

    msg = b''
    msg += pack('I', len(token))
    msg += token
    msg += pack('III', session_size, 1, 2)

    ip = inet_aton(public_ip)
    ip = bytearray(ip)
    ip.reverse()

    msg += ip
    msg += pack('III', 0, int(session_time), 1)
    
    return msg

def steam_login(user, password):
    global game_tokens

    app_id = 1384160 # Strive appId
    client = SteamClient()
    client.on(EMsg.ClientGameConnectTokens, on_game_tokens)
    session_time = time()
    client.cli_login(user, password)

    print("Successfully logged in")

    app_ticket = client.get_app_ticket(app_id).ticket
    auth_ticket = create_auth_ticket(game_tokens[0], time() - session_time)
    crc = crc32(auth_ticket)

    message = MsgProto(EMsg.ClientAuthList)
    message.body.tokens_left = len(game_tokens)
    message.body.app_ids.extend([app_id])

    tickets = message.body.tickets.add()

    # wtf is this proto bullshit.
    ticket = {
        'gameid': app_id,
        'ticket': auth_ticket,
        'ticket_crc': crc
    }

    proto_fill_from_dict(tickets, ticket)

    resp = client.send_message_and_wait(message, EMsg.ClientAuthListAck)
    #print(resp)
    # build login token
    msg = auth_ticket
    msg += pack('I', len(app_ticket))
    msg += app_ticket

    return {
        'id': client.user.steam_id.as_64,
        'token': hexlify(msg).decode().upper()
    }

def login(user, password, auth=None, padding=0):
    if not auth:
        auth = steam_login(user, password)

    steam_id = auth['id']
    steam_hex = hex(steam_id)[:2]
    #print(auth)

    msg = [
        [
            "",
            "",
            2,
            "0.2.1",
            3
        ],
        [
            1,
            steam_id,
            steam_hex,
            256,
            auth['token']
        ]
    ]

    encrypted = encrypt_request_data(msg)

    r = post(
        r'https://ggst-game.guiltygear.com/api/user/login',
        headers={
            'Cache-Control': r'no-store',
            'Content-Type': r'application/x-www-form-urlencoded',
            'User-Agent': r'GGST/Steam',
            'x-client-version': r'1',
            'authority': 'ggst-game.guiltygear.com'
        },
        data={
            'data': encrypted if padding >= 0 else encrypted[:padding]
        },
    )

    try:
        login_response = decrypt_response_data(r.content.hex())
    except:
        return login(user, password, auth, padding - 2)

    print(login_response);

    #This is where I print it?
    token = login_response[0][0]
    print(f"Strive token obtained for user: {steam_id} - {token}")
    file = open("token.txt", "wb")
    file.write(token)
    file.close()
    return token

key = unhexlify('EEBC1F57487F51921C0465665F8AE6D1658BB26DE6F8A069A3520293A572078F')

def get_replays(token):
    data_header = [
        "230129212655563979",
        token,
        2,
        "0.2.1",
        3
    ]
    data_params = [
        1,
        0,
        127,
        [
            -1,
            0,
            1,
            99,
            [],
            -1,
            -1,
            0,
            0,
            1
        ],
        6  # all platforms? (3 = PC?)
    ]
    msg = [data_header, data_params]
    return post_api("catalog/get_replay", msg)

def post_api(endpoint, msg):
    r = post(
        f'https://ggst-game.guiltygear.com/api/{endpoint}',
        headers={
            'Cache-Control': r'no-store',
            'Content-Type': r'application/x-www-form-urlencoded',
            'User-Agent': r'GGST/Steam',
            'x-client-version': r'1',
        },
        data={
            'data': encrypt_request_data(msg)
        },
    )

    content = r.content
    return decrypt_response_data(content.hex())


def encrypt_request_data(data):
    msg = packb(data)
    iv = get_random_bytes(12)
    cipher = AES.new(key, AES.MODE_GCM, iv)
    encrypted = cipher.encrypt(msg)
    tag = cipher.digest()
    encrypted = hexlify(iv + encrypted + tag)
    return urlsafe_b64encode(unhexlify(encrypted))


def decrypt_response_data(data):
    decoded = unhexlify(data)
    iv = decoded[:12]
    cipher = AES.new(key, AES.MODE_GCM, iv)
    decrypted = cipher.decrypt(decoded[12:])
    return unpackb(decrypted[:-16])

if __name__ == "__main__":
    main()
