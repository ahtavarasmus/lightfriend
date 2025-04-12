import hmac
import hashlib

nonce = "f5bd3873d3ab11878e6e62eface0b8dd79bf1cc6b3e1f0be7bfe4f3d74d50398538c5b96412227721cef0948e4565181cb7bcd71a075509f892259d510e18972"
username = "appuser_75a846412f734a189d583bc929f9dea9"
password = "f1fcd8e0-3f65-4a8a-89d4-ae0917505ee6"
admin = "false"
shared_secret = "testsecret123"

mac_content = f"{nonce}\0{username}\0{password}\0{admin}"
mac = hmac.new(shared_secret.encode(), mac_content.encode(), hashlib.sha1)
mac_hex = mac.hexdigest()

print(f"MAC content: {mac_content}")
print(f"MAC: {mac_hex}")
