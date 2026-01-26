curl -s http://127.0.0.1:8899 \
  -H "content-type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"surfnet_resetNetwork","params":[]}'
