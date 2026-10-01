import urllib.request

req = urllib.request.Request('https://www.rust-lang.org/', headers={'User-Agent': 'Mozilla/5.0'})
raw = urllib.request.urlopen(req).read().decode('utf-8')
idx = raw.find('class="select"')
print(raw[idx-50:idx+450])
