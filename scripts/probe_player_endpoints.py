"""Probe SofaScore endpoints for player season stats."""
from curl_cffi import requests

s = requests.Session(impersonate="chrome")
TEAM = 22006
UT = 242
SEASON = 86668

candidates = [
    f"https://api.sofascore.com/api/v1/team/{TEAM}/unique-tournament/{UT}/season/{SEASON}/statistics/overall",
    f"https://api.sofascore.com/api/v1/unique-tournament/{UT}/season/{SEASON}/statistics?limit=60",
    f"https://api.sofascore.com/api/v1/team/{TEAM}/players",
    f"https://api.sofascore.com/api/v1/squad/{TEAM}",
    f"https://api.sofascore.com/api/v1/team/{TEAM}/player-statistics?seasonId={SEASON}",
]
for url in candidates:
    try:
        r = s.get(url, timeout=30)
        head = r.text[:120].replace("\n", " ")
        print(r.status_code, url.split("api/v1/")[1][:70], "|", head)
    except Exception as e:
        print("ERR", url, e)
