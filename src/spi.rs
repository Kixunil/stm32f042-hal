use core::ptr;

use nb;

pub use hal::spi::{Mode, Phase, Polarity};
use rcc::Clocks;

use stm32f042::{SPI1, RCC};

use gpio::gpioa::{PA5, PA6, PA7};
use gpio::gpiob::{PB2, PB4, PB5};
use gpio::{AF0, Alternate};
use time::{Hertz};

/// SPI error
#[derive(Debug)]
pub enum Error {
    /// Overrun occurred
    Overrun,
    /// Mode fault occurred
    ModeFault,
    /// CRC error
    Crc,
    #[doc(hidden)]
    _Extensible,
}

/// SPI abstraction
pub struct Spi<SPI, PINS> {
    spi: SPI,
    pins: PINS,
}

pub trait Pins<Spi> {}

impl Pins<SPI1>
    for (
        PA5<Alternate<AF0>>,
        PA6<Alternate<AF0>>,
        PA7<Alternate<AF0>>,
    ) {
}
impl Pins<SPI1>
    for (
        PB2<Alternate<AF0>>,
        PB4<Alternate<AF0>>,
        PB5<Alternate<AF0>>,
    ) {
}

impl<PINS> Spi<SPI1, PINS> {
    pub fn spi1(spi: SPI1, pins: PINS, mode: Mode, speed: Hertz, clocks: Clocks) -> Self
    where
        PINS: Pins<SPI1>,
    {
        // NOTE(unsafe) This executes only during initialisation
        let rcc = unsafe { &(*RCC::ptr()) };

        /* Enable clock for SPI1 */
        rcc.apb2enr.modify(|_, w| w.spi1en().set_bit());

        /* Reset SPI1 */
        rcc.apb2rstr.modify(|_, w| w.spi1rst().set_bit());
        rcc.apb2rstr.modify(|_, w| w.spi1rst().clear_bit());

        /* Make sure the SPI unit is disabled so we can configure it */
        spi.cr1.modify(|_, w| w.spe().clear_bit());

        // disable SS output
        spi.cr2.write(|w| w.ssoe().clear_bit());

        let br = match clocks.pclk().0 / speed.0 {
            0 => unreachable!(),
            1...2 => 0b000,
            3...5 => 0b001,
            6...11 => 0b010,
            12...23 => 0b011,
            24...47 => 0b100,
            48...95 => 0b101,
            96...191 => 0b110,
            _ => 0b111,
        };

        // mstr: master configuration
        // lsbfirst: MSB first
        // ssm: enable software slave management (NSS pin free for other uses)
        // ssi: set nss high = master mode
        // dff: 8 bit frames
        // bidimode: 2-line unidirectional
        // spe: enable the SPI bus
        spi.cr1.write(|w| unsafe {
            w.cpha()
                .bit(mode.phase == Phase::CaptureOnSecondTransition)
                .cpol()
                .bit(mode.polarity == Polarity::IdleHigh)
                .mstr()
                .set_bit()
                .br()
                .bits(br)
                .lsbfirst()
                .clear_bit()
                .ssm()
                .set_bit()
                .ssi()
                .set_bit()
                .rxonly()
                .clear_bit()
                .bidimode()
                .clear_bit()
                .spe()
                .set_bit()
        });

        Spi { spi, pins }
    }

    pub fn release(self) -> (SPI1, PINS) {
        (self.spi, self.pins)
    }
}

impl<PINS> ::hal::spi::FullDuplex<u8> for Spi<SPI1, PINS> {
    type Error = Error;

    fn read(&mut self) -> nb::Result<u8, Error> {
        let sr = self.spi.sr.read();

        Err(if sr.ovr().bit_is_set() {
            nb::Error::Other(Error::Overrun)
        } else if sr.modf().bit_is_set() {
            nb::Error::Other(Error::ModeFault)
        } else if sr.crcerr().bit_is_set() {
            nb::Error::Other(Error::Crc)
        } else if sr.rxne().bit_is_set() {
            // NOTE(read_volatile) read only 1 byte (the svd2rust API only allows
            // reading a half-word)
            return Ok(unsafe { ptr::read_volatile(&self.spi.dr as *const _ as *const u8) });
        } else {
            // FIXME: Should use WouldBlock in case of reading is supposed to work
            //nb::Error::WouldBlock
            return Ok(0);
        })
    }

    fn send(&mut self, byte: u8) -> nb::Result<(), Error> {
        let sr = self.spi.sr.read();

        Err(if sr.ovr().bit_is_set() {
            nb::Error::Other(Error::Overrun)
        } else if sr.modf().bit_is_set() {
            nb::Error::Other(Error::ModeFault)
        } else if sr.crcerr().bit_is_set() {
            nb::Error::Other(Error::Crc)
        } else if sr.txe().bit_is_set() {
            // NOTE(write_volatile) see note above
            unsafe { ptr::write_volatile(&self.spi.dr as *const _ as *mut u8, byte) }
            return Ok(());
        } else {
            nb::Error::WouldBlock
        })
    }
}

impl<PINS> ::hal::blocking::spi::transfer::Default<u8> for Spi<SPI1, PINS> {}
impl<PINS> ::hal::blocking::spi::write::Default<u8> for Spi<SPI1, PINS> {}
