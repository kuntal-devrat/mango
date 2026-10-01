import urllib.request, re, html

req = urllib.request.Request('https://en.wikipedia.org/wiki/Main_Page', headers={'User-Agent': 'Mozilla/5.0'})
raw = urllib.request.urlopen(req).read().decode('utf-8')
links = [html.unescape(l) for l in re.findall(r'<link[^>]+rel=[\'"]stylesheet[\'"][^>]+href=[\'"]([^\'"]+)[\'"]', raw)]
for l in links:
    if l.startswith('//'): l = 'https:' + l
    elif l.startswith('/'): l = 'https://en.wikipedia.org' + l
    try:
        css = urllib.request.urlopen(urllib.request.Request(l, headers={'User-Agent': 'Mozilla/5.0'})).read().decode('utf-8')
        for sel in ['mp-itn', 'vector-pinned-container', 'content-list']:
            for m in re.finditer(r'([^{}]*' + sel + r'[^{}]*)\{([^}]+)\}', css):
                print(m.group(1).strip() + ' => ' + m.group(2).strip()[:100])
    except Exception as e:
        print(e)
