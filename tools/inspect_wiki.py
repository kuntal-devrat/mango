import urllib.request
import re

req = urllib.request.Request(
    'https://en.wikipedia.org/wiki/Main_Page',
    headers={'User-Agent': 'Mozilla/5.0'}
)
html = urllib.request.urlopen(req).read().decode('utf-8')

idx = html.find('id="vector-appearance"')
print("--- EXACT ID VECTOR APPEARANCE ---")
if idx != -1:
    print(html[idx-100:idx+600])
else:
    print("id=\"vector-appearance\" NOT FOUND")

idx2 = html.find('vector-appearance-pinnable-header')
print("--- PINNABLE HEADER ---")
if idx2 != -1:
    print(html[idx2-100:idx2+600])

itn_ul = html.find('id="mp-itn"')
if itn_ul != -1:
    ul_start = html.find('<ul', itn_ul)
    print("\n--- MP-ITN UL AND FIRST LI ---")
    print(html[ul_start:ul_start+800])

print("\n--- IN THE NEWS LIST ITEMS ---")
itn_idx = html.find('id="mp-itn"')
if itn_idx != -1:
    print(html[itn_idx:itn_idx+1200])

print("\n--- ACTION TABS (Main Page / Read / View history) ---")
tab_idx = html.find('id="ca-nstab-main"')
if tab_idx != -1:
    print("--- ANCESTORS OF CA-NSTAB-MAIN ---")
    print(html[tab_idx-400:tab_idx])
