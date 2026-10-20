import urllib.request, re

req = urllib.request.Request('https://www.rust-lang.org/', headers={'User-Agent': 'Mozilla/5.0'})
raw = urllib.request.urlopen(req).read().decode('utf-8')
for m in re.finditer(r'<link[^>]+rel=[\'"]stylesheet[\'"][^>]+href=[\'"]([^\'"]+)[\'"]', raw):
    l = m.group(1)
    if l.startswith('/'): l = 'https://www.rust-lang.org' + l
    try:
        css = urllib.request.urlopen(urllib.request.Request(l, headers={'User-Agent': 'Mozilla/5.0'})).read().decode('utf-8')
        for match in re.finditer(r'([^{}]*select[^{}]*)\{([^}]+)\}', css):
            print(match.group(1).strip() + ' => ' + match.group(2).strip())
    except Exception as e:
        print(e)
