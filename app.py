from flask import Flask, send_file, jsonify

app = Flask(__name__)


@app.route("/", methods=["GET"])
def index():
    return send_file("boogle.html"), 200


@app.route("/jsontest", methods=["GET"])
def jsontest():
    return jsonify({"data": "I am in CSE190/CSE291!"}), 200


if __name__ == "__main__":
    app.run(host="0.0.0.0", port=5000)
