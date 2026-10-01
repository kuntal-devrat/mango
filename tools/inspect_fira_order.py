import urllib.request, re

req = urllib.request.Request('https://www.rust-lang.org/', headers={'User-Agent': 'Mozilla/5.0'})
raw = urllib.request.urlopen(req).read().decode('utf-8')
links = re.findall(r'<link[^>]+rel=[\'"]stylesheet[\'"][^>]+href=[\'"]([^\'"]+)[\'"]', raw)
css_all = ''
for l in links:
    if l.startswith('/'): l = 'https://www.rust-lang.org' + l
    css_all += urllib.request.urlopen(urllib.request.Request(l, headers={'User-Agent': 'Mozilla/5.0'})).read().decode('utf-8')

ffs = re.findall(r'@font-face\s*\{([^}]+)\}', css_all)
for ff in ffs:
    if 'Fira Sans' in ff:
        weight = re.search(r'font-weight:\s*(\d+)', ff)
        style = re.search(r'font-style:\s*(\w+)', ff)
        src = re.search(r'url\([\'"]?([^\'"\)]+)', ff)
        urange = re.search(r'unicode-range:\s*([^;]+)', ff)
        print(f"weight: {weight.group(1) if weight else None}, style: {style.group(1) if style else None}, src: {src.group(1) if src else None}")
