import urllib.request, re

req = urllib.request.Request('https://en.wikipedia.org/wiki/Main_Page', headers={'User-Agent': 'Mozilla/5.0'})
raw = urllib.request.urlopen(req).read().decode('utf-8')
for s in re.finditer(r'<style[^>]*>([^<]+)</style>', raw):
    body = s.group(1)
    for m in re.finditer(r'([^{}]*mp-(?:upper|left|right)[^{}]*)\{([^}]+)\}', body):
        print(m.group(1).strip() + ' => ' + m.group(2).strip())
