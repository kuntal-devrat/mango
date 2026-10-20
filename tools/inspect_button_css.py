import urllib.request, re

req = urllib.request.Request('https://www.rust-lang.org/', headers={'User-Agent': 'Mozilla/5.0'})
raw = urllib.request.urlopen(req).read().decode('utf-8')
links = re.findall(r'<link[^>]+rel=[\'"]stylesheet[\'"][^>]+href=[\'"]([^\'"]+)[\'"]', raw)
css_all = ''
for l in links:
    if l.startswith('/'): l = 'https://www.rust-lang.org' + l
    css_all += urllib.request.urlopen(urllib.request.Request(l, headers={'User-Agent': 'Mozilla/5.0'})).read().decode('utf-8')

print('--- .button RULES ---')
for m in re.finditer(r'([^{}]*\.button[^{}]*)\{([^}]+)\}', css_all):
    print(m.group(1).strip() + ' => ' + m.group(2).strip())
