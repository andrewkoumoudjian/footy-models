"""Direct selenium test: can headless chromium load FBref at all?"""
import sys
from selenium import webdriver
from selenium.webdriver.chrome.options import Options
from selenium.webdriver.chrome.service import Service

opts = Options()
opts.binary_location = '/usr/bin/chromium'
opts.add_argument('--headless=new')
opts.add_argument('--no-sandbox')
opts.add_argument('--disable-dev-shm-usage')
opts.add_argument('--disable-gpu')
opts.add_argument('--user-data-dir=/tmp/chrome-fbref-test')
opts.add_argument('user-agent=Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0 Safari/537.36')

try:
    svc = Service('/usr/bin/chromedriver')
    drv = webdriver.Chrome(service=svc, options=opts)
    drv.set_page_load_timeout(60)
    drv.get('https://fbref.com/en/comps/9/2024-2025/standard/2024-2025-Premier-League-Stats')
    import time; time.sleep(8)
    title = drv.title
    html = drv.page_source
    print('title:', title)
    print('bytes:', len(html))
    print('has stats table:', 'stats_standard' in html or 'data-append-csv' in html)
    print('blocked:', 'Attention Required' in html or 'cf-chl' in html)
    drv.quit()
except Exception as e:
    print('ERROR:', type(e).__name__, str(e)[:300])
    sys.exit(1)
