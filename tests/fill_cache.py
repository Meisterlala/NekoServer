import requests
import random
import os
import time

url = os.getenv('NEKO_SERVER', 'http://127.0.0.1') + "/count/"
timing = []

# Fill the cache
for i in range(50000):
    response = requests.get(url + str(random.randint(0, 100000000)))
    if (response.status_code != 200):
        print("Error: " + response.text)
    elif (i % 100 == 0):
        print(".", end="", flush=True)
    timing.append(response.elapsed.total_seconds())
    del response

print("\nAverage response time: " + str(sum(timing) / len(timing) * 1000) + "ms")