import asyncio
import websockets
import json
import uuid

# Configuration
BASE_URL = "ws://127.0.0.1:3000"
GAME_ID = str(uuid.uuid4())
PLAYER_ID = str(uuid.uuid4())
CF_HANDLE = "test_user"

async def test_reconnection():
    print(f"Testing Game: {GAME_ID}, Player: {PLAYER_ID}")
    
    # 1. Create Game (via API usually, but let's assume game exists or we create it first)
    # Actually, we need to crate the game first via HTTP
    import requests
    api_url = "http://127.0.0.1:3000/api/game"
    try:
        resp = requests.post(api_url, json={
            "player_id": PLAYER_ID,
            "cf_handle": CF_HANDLE,
            "game_config": {
                "difficulty": "Easy",
                "heat_threshold": 7,
                "game_duration_mins": 30
            }
        })
        print(f"Create Game Response: {resp.status_code} {resp.text}")
        game_data = resp.json()
        game_id = game_data['game_id']
    except Exception as e:
        print(f"Failed to create game: {e}")
        return

    # 2. First Connection
    uri = f"{BASE_URL}/ws/{game_id}?player_id={PLAYER_ID}"
    print(f"Connecting to {uri}...")
    
    try:
        async with websockets.connect(uri) as websocket:
            # Send Join
            join_msg = {
                "type": "JoinGame",
                "player_id": PLAYER_ID,
                "cf_handle": CF_HANDLE
            }
            await websocket.send(json.dumps(join_msg))
            print("Sent JoinGame")
            
            # Wait for response
            response = await websocket.recv()
            print(f"Received: {response}")
            
            # Wait a bit
            await asyncio.sleep(1)
            print("Disconnecting...")
    except Exception as e:
        print(f"Connection 1 Failed: {e}")

    # 3. Simplify Reload (Second Connection)
    print("Reconnecting (Simulating Reload)...")
    try:
        async with websockets.connect(uri) as websocket:
             # Send Join Again (Simulate frontend hook re-running)
            join_msg = {
                "type": "JoinGame",
                "player_id": PLAYER_ID,
                "cf_handle": CF_HANDLE
            }
            await websocket.send(json.dumps(join_msg))
            print("Sent JoinGame (Reconnect)")
            
            # Expect GameJoined AND GameUpdate
            while True:
                try:
                    response = await asyncio.wait_for(websocket.recv(), timeout=2.0)
                    print(f"Received on Reconnect: {response}")
                except asyncio.TimeoutError:
                    print("No more messages")
                    break
    except Exception as e:
        print(f"Connection 2 Failed: {e}")

if __name__ == "__main__":
    asyncio.run(test_reconnection())
