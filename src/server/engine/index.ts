import { show } from "./show";

class Supervisor {
    start() {
        show.load()
    }
}

export default new Supervisor()