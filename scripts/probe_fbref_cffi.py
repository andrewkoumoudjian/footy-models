"""Test FBref access with curl_cffi Chrome impersonation (bypasses TLS fingerprint block)."""
import re, sys
try:
    from curl_cffi import requests as creq
except ImportError:
    print('curl_cffi not installed')
    sys.exit(2)

url = 'https://fbref.com/en/comps/9/2024-2025/standard/2024-2025-Premier-League-Stats'
try:
    r = creq.get(url, impersonate='chrome', timeout=45)
    print('status:', r.status_code, 'bytes:', len(r.text))
    if r.status_code == 200:
        n = len(re.findall(r'data-append-csv="[^"]+"', r.text))
        print('player rows:', n)
        print('Haaland in page:', 'Haaland' in r.text)
except Exception as e:
    print('ERROR:', e)
    sys.exit(1)
