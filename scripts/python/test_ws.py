import asyncio
import json
from random import random

import requests
import websockets

API_URL = "http://localhost:8080/"


def generate_random_username():
    return f"testuser{int(random() * 10000)}"


def create_user(username, password):
    create_user_response = requests.post(
        f"{API_URL}/auth/signup",
        json={
            "name": username,
            "password": password,
            "email": f"{username}@gmail.com",
        },
    )
    if create_user_response.status_code != 201:
        raise Exception(
            f"Failed to create user: {create_user_response.status_code} - {create_user_response.text}"
        )

    create_user_response_json = create_user_response.json()
    return (
        create_user_response_json["token"],
        create_user_response_json["refresh_token"],
    )


def get_user_id(token):
    user_info_response = requests.get(
        f"{API_URL}/auth/me",
        headers={"Authorization": f"Bearer {token}"},
    )
    if user_info_response.status_code != 200:
        raise Exception(
            f"Failed to get user info: {user_info_response.status_code} - {user_info_response.text}"
        )

    user_response_json = user_info_response.json()
    id = user_response_json["id"]
    full_id = f"{id['tb']}:{id['id']['String']}"
    return full_id


def refresh_token(refresh_token):
    refresh_token_response = requests.post(
        f"{API_URL}/auth/refresh",
        json={
            "refresh_token": refresh_token,
        },
    )
    if refresh_token_response.status_code != 200:
        raise Exception(
            f"Failed to refresh token: {refresh_token_response.status_code} - {refresh_token_response.text}"
        )

    refresh_token_response_json = refresh_token_response.json()
    return refresh_token_response_json["token"], refresh_token_response_json[
        "refresh_token"
    ]


async def create_two_users():
    user1 = generate_random_username()
    user2 = generate_random_username()

    while user2 == user1:
        user2 = generate_random_username()

    password = "testpassword"

    user1_token, user1_refresh = create_user(user1, password)
    user2_token, user2_refresh = create_user(user2, password)

    user1_id = get_user_id(user1_token)
    user2_id = get_user_id(user2_token)

    return {
        "user1": {"token": user1_token, "refresh": user1_refresh, "id": user1_id},
        "user2": {"token": user2_token, "refresh": user2_refresh, "id": user2_id},
    }


async def main():
    users_dict = await create_two_users()
    uri = "ws://localhost:8080/ws"

    async def listen(ws, user, queue):
        try:
            async for raw in ws:
                try:
                    message = json.loads(raw)
                except json.JSONDecodeError:
                    message = raw
                print(f"{user} received: {message}")
                await queue.put(message)
        except websockets.ConnectionClosed:
            print(f"{user} connection closed")

    async def send(ws, payload):
        await ws.send(json.dumps(payload))

    async def wait_for_type(queue, msg_type, timeout=5):
        """Consomme la queue jusqu'à trouver un message du bon type."""
        deadline = asyncio.get_event_loop().time() + timeout
        while True:
            remaining = deadline - asyncio.get_event_loop().time()
            if remaining <= 0:
                raise asyncio.TimeoutError(f"No {msg_type} received")
            msg = await asyncio.wait_for(queue.get(), timeout=remaining)
            if isinstance(msg, dict) and msg.get("message_type") == msg_type:
                return msg

    async with websockets.connect(uri) as ws1, websockets.connect(uri) as ws2:
        print("WebSocket connections established")

        q1, q2 = asyncio.Queue(), asyncio.Queue()

        l1 = asyncio.create_task(listen(ws1, "user1", q1))
        l2 = asyncio.create_task(listen(ws2, "user2", q2))

        # Auth
        await send(
            ws1,
            {
                "user_id": users_dict["user1"]["id"],
                "token": users_dict["user1"]["token"],
            },
        )
        await send(
            ws2,
            {
                "user_id": users_dict["user2"]["id"],
                "token": users_dict["user2"]["token"],
            },
        )
        await asyncio.sleep(0.2)

        # Échange initial
        await send(
            ws1,
            {
                "to": users_dict["user2"]["id"],
                "message_type": "dm_message",
                "from": users_dict["user1"]["id"],
                "content": "Hello from user1!",
            },
        )
        await send(
            ws2,
            {
                "to": users_dict["user1"]["id"],
                "from": users_dict["user2"]["id"],
                "message_type": "dm_message",
                "content": "Hello from user2!",
            },
        )

        msg = await wait_for_type(q1, "dm_channel_created", timeout=5)
        channel_id = msg["content"]
        print(f"DM channel created with ID: {channel_id}")

        while not q2.empty():
            q2.get_nowait()

        await send(
            ws1,
            {
                "message_type": "dm_message",
                "to": channel_id,
                "from": users_dict["user1"]["id"],
                "content": "This is a message in the DM channel!",
            },
        )

        channel_msg = await wait_for_type(q2, "dm_message", timeout=5)
        print(f"user2 received channel message: {channel_msg}")

        await asyncio.sleep(0.5)

        l1.cancel()
        l2.cancel()


if __name__ == "__main__":
    asyncio.run(main())
