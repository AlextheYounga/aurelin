if redis.call("get", KEYS[1]) ~= ARGV[1] then
    return 0
end

if tonumber(ARGV[2]) > 0 then
    return redis.call("pexpire", KEYS[1], ARGV[2])
end

redis.call("persist", KEYS[1])
return 1
