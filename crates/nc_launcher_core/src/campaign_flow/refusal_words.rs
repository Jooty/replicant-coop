use replicant_game::story_progress::SeatError;
use replicant_proto::SeatRefusal;

pub fn seat_refusal_words(reason: SeatRefusal) -> &'static str {
    match reason {
        SeatRefusal::WrongSize => "it arrived with the wrong size",
        SeatRefusal::OutOfOrder => "its parts arrived out of order",
        SeatRefusal::BadChecksum => "it was damaged on the way",
        SeatRefusal::NotASave => "it is not a save the game can load",
    }
}

pub fn seat_error_words(error: SeatError) -> &'static str {
    match error {
        SeatError::FileSizeWrong => "the save file has the wrong size",
        SeatError::SlotSizeWrong => "the save has the wrong size",
        SeatError::NoSuchSlot => "the save file has no such slot",
        SeatError::NotAGameSlot => "the slot is not a save the game wrote",
        SeatError::EmptySlot => "the slot is empty",
        SeatError::ChecksumWrong => "the save's checksum is wrong",
    }
}
