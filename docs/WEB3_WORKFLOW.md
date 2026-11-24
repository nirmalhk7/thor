# Web3 workflow: check which account an Ethereum call names

When two test accounts return different token balances, confirm that the
request is querying the intended account. Open **Cmd+Shift+K**, search
**decode calldata**, and choose **Inspect an Ethereum call**. Paste the
[local balance request fixture](../tests/fixtures/web3_balance_call.json),
or the JSON-RPC body of your own captured `eth_call`, then choose **Inspect call**.

The fixture names a contract ending in `1111` and an account ending in
`2222`. The first address is the call recipient. The second is the decoded
argument of [`balanceOf(address)`](https://eips.ethereum.org/EIPS/eip-20#balanceof).
Thor inspects the provided transaction object in the
[`eth_call` request](https://ethereum.org/developers/docs/apis/json-rpc/#eth_call).
It does not execute the call, verify the contract's ABI, or fetch a balance.

This first workflow decodes `balanceOf(address)` only. Other well-formed
selectors are displayed without guessed function names or arguments. Malformed
or ambiguous input is rejected. Use request JSON only; do not include headers,
credentials, or signing requests.
