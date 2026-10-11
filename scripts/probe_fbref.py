"""Probe FBref player-season stats table via plain HTTP (server-rendered HTML)."""
import re, sys
import requests

UA = {'User-Agent': 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0 Safari/537.36',
      'Accept-Language': 'en-US,en;q=0.9'}

# Premier League 2024-25 standard stats page
url = 'https://fbref.com/en/comps/9/2024-2025/standard/2024-2025-Premier-League-Stats'
try:
    r = requests.get(url, headers=UA, timeout=30)
    print('status:', r.status_code, 'bytes:', len(r.text))
    if r.status_code == 200:
        # count player rows
        n = len(re.findall(r'data-append-csv="[^"]+"', r.text))
        print('player rows found:', n)
        # check a known player name appears (Konate / Haaland)
        for probe in ['Haaland', 'Saka', 'Palmer']:
            print(probe, 'in page:', probe in r.text)
        # extract first few data-append-csv ids
        ids = re.findall(r'data-append-csv="([^"]+)"', r.text)[:5]
        print('sample player ids:', ids)
except Exception as e:
    print('ERROR:', e)
    sys.exit(1)
