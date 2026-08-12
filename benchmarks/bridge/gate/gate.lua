-- gate.lua — Gate A + B for Lua. args: cpath, runtime seed.
-- N is scaled: native Lua feature_hash is interpreted (~30us/call), so it gets
-- fewer iterations than the C-native Briev path.
package.cpath = package.cpath .. ';' .. arg[1] .. '/?.so'
local b = require('bench')

local function native_fh(count, seed)
    local h = seed
    for i = 0, count - 1 do
        h = (h ~ (i * 2654435761)) * 1099511628211
    end
    return h
end
local function native_add(a, c) return a + c end

local r = tonumber(arg[2])
b.feature_hash(1000, r)
local N, N2, Nn = 200000, 2000000, 2000
local t0 = os.clock()
local sink = 0
for i = 1, N do sink = sink + b.feature_hash(1000, r) end
print(string.format('BRIEV_FH %.1f', (os.clock() - t0) * 1e9 / N))
native_fh(1000, r)
t0 = os.clock()
for i = 1, Nn do sink = sink + native_fh(1000, r + i) end
print(string.format('NATIVE_FH %.1f', (os.clock() - t0) * 1e9 / Nn))
b.add(r, 4)
t0 = os.clock()
for i = 1, N2 do sink = sink + b.add(r, 4) end
print(string.format('BRIEV_ADD %.2f', (os.clock() - t0) * 1e9 / N2))
native_add(r, 4)
t0 = os.clock()
for i = 1, N2 do sink = sink + native_add(r, i % 8) end
print(string.format('NATIVE_ADD %.2f', (os.clock() - t0) * 1e9 / N2))
_ = sink
