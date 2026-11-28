#![no_std]

//! Reparto de cada lectura de PaperPay en la misma transacción del pago:
//! la editorial recibe el monto menos la comisión y la tesorería de PaperPay recibe la comisión
//! (`fee_bps`, 200 = 2%). Los pagos no pasan por el contrato: mueve el USDC directo de la cuenta
//! de quien paga a las cuentas de destino. Cada artículo pagado emite un evento `paid` con su id
//! y con quién lo puede leer, que queda en la red como comprobante.
//!
//! Seis formas de pagar una lectura, con la misma comisión:
//! - `pay`: un artículo, para quien paga.
//! - `pay_batch`: varios artículos (hasta `MAX_BATCH_ITEMS`) con una sola firma; todo o nada.
//! - `gift`: un artículo que paga una cuenta y lee otra.
//! - `gift_batch`: varios artículos que paga una cuenta y lee otra (una lista de lectura).
//! - `pay_authors`: un artículo cuya parte de editorial se reparte entre coautores.
//! - `pay_ref`: un artículo recomendado por otra cuenta, que se lleva la mitad de la comisión.
//!
//! Accesos con vencimiento, que guardan su fecha en el contrato (sin guardar fondos):
//! - `buy_pass`: un pase de N días al catálogo de una editorial, a precio fijo por día.
//! - `subscribe`: el mismo pase, que se renueva solo: cobra el primer periodo y le da al contrato
//!   una autorización (allowance del token) por el precio de las renovaciones.
//! - `renew_subscription`: cobra una renovación con esa autorización, sin firma de quien lee.
//!   Cualquiera la puede enviar, pero solo cobra el precio pactado y solo cuando al pase le falta
//!   un día o menos. Los fondos siguen en la wallet de quien lee hasta cada cobro.
//! - `sponsor_open`: alguien paga para que un artículo quede abierto para todos N días.
//!
//! Campañas de acceso abierto, la primera custodia del contrato:
//! - `open_campaign`: la editorial ofrece liberar un artículo para todos si los aportes llegan a
//!   una meta antes de un plazo.
//! - `pledge`: un aporte hacia la meta. Queda guardado en el contrato hasta que se cumple la meta
//!   (va a la editorial) o vence el plazo sin cumplirla (vuelve a quien aportó). No hay admin: ese
//!   dinero no tiene otra salida.
//! - `release_pledges`: con la meta cumplida, cualquiera la ejecuta; la editorial cobra el 98% y la
//!   tesorería el 2%, y el artículo queda liberado.
//! - `refund_pledge`: si vence el plazo sin llegar a la meta, cualquiera la ejecuta y cada aporte
//!   vuelve completo a quien lo hizo.
//!
//! Fondos de patrocinio, la segunda custodia: alguien deja dinero para pagar lecturas de otros.
//! - `fund_pool`: un patrocinador deposita para las lecturas de una editorial y nombra a un
//!   operador, con un tope por lectura.
//! - `pay_sponsored`: el operador paga una lectura con el fondo, para un lector que no paga ni
//!   firma. Solo puede pagarle a la editorial del fondo, hasta el tope, y una vez por artículo y
//!   lector cada 24 h.
//! - `withdraw_pool`: el patrocinador retira lo que no se usó, cuando quiera.
//!
//! Recompensas por revisar o reproducir un artículo, la tercera custodia:
//! - `post_bounty`: alguien deja una recompensa en garantía, con un plazo.
//! - `award_bounty`: quien la publicó la reparte entre quienes hicieron el trabajo (98%) y la
//!   tesorería (2%).
//! - `cancel_bounty`: si nadie la ganó, quien la publicó la recupera, pero solo después del
//!   plazo: mientras tanto, quien trabaja sabe que el dinero está ahí.
//!
//! Vales de lectura, la cuarta custodia: lecturas prepagadas que se reparten como enlaces.
//! - `buy_vouchers`: alguien paga por adelantado varios vales para el catálogo de una editorial.
//!   Cada vale es una llave ed25519; el contrato solo guarda la parte pública.
//! - `redeem_voucher`: quien tiene la llave de un vale firma qué artículo quiere y para qué
//!   lector. Con esa firma cualquiera envía el canje: el contrato paga la lectura con el vale.
//! - `reclaim_vouchers`: quien compró recupera los vales que vencieron sin usarse.
//!
//! Cuenta abierta, la quinta custodia: leer ahora y pagar todo junto con una firma.
//! - `open_tab`: quien lee deja una garantía y nombra a un operador (el sitio).
//! - `charge_tab` (de soporte, no mueve fondos): el operador anota cada lectura como deuda con su
//!   editorial. La deuda nunca pasa de la garantía.
//! - `settle_tab`: quien lee paga toda la deuda con una firma; cada editorial cobra su 98%.
//! - `collect_tab`: si no pagó a tiempo, cualquiera cobra la deuda de la garantía.
//! - `close_tab`: sin deuda, quien lee recupera su garantía.
//!
//! Pase PaperPay, la sexta custodia: un pase para todo el catálogo, que se reparte entre las
//! editoriales según lo que esa persona leyó.
//! - `buy_global_pass`: quien lee paga el pase; el dinero queda en el contrato hasta que vence.
//! - `use_pass` (de soporte, no mueve fondos): el operador anota cada lectura hecha con el pase.
//! - `settle_pass`: vencido el pase, cualquiera lo liquida: 2% a la tesorería y el resto entre las
//!   editoriales, en proporción a las lecturas. Si no leyó nada, vuelve todo.
//! - `cancel_pass`: mientras no haya leído nada, quien lo compró lo cancela y recupera todo.
//!
//! Journal club, la séptima custodia: un grupo junta una caja común y paga lo que aprueba.
//! - `create_club` (de soporte): alguien define la cuota y cuántas aprobaciones hacen falta.
//! - `join_club`: cada miembro paga su cuota a la caja del club.
//! - `propose_read` (de soporte): un miembro propone pagar un artículo con la caja.
//! - `approve_read`: otro miembro aprueba; la aprobación que completa el quórum paga a la
//!   editorial (98%) y a la tesorería (2%) desde la caja.
//! - `leave_club`: quien sale se lleva una parte igual de lo que queda en la caja.
//!
//! Pago contra entrega, la octava custodia: la editorial solo cobra si entrega la llave.
//! - `lock_payment`: quien lee deja el pago en el contrato, atado al sha256 de la llave que abre
//!   el artículo, con un plazo.
//! - `unlock_payment`: quien presenta la llave antes del plazo libera el pago: la editorial cobra
//!   el 98% y la tesorería el 2%, y la llave queda publicada en el evento.
//! - `reclaim_payment`: si la llave no llegó antes del plazo, quien pagó recupera todo.
//!
//! Compromiso del revisor, la novena custodia, sobre las recompensas: quien va a hacer el trabajo
//! pone algo en juego.
//! - `accept_bounty`: toma una recompensa y deja una garantía (al menos el 10% de la recompensa).
//! - `submit_review`: si entrega antes del plazo (el hash de su informe), recupera la garantía.
//! - `forfeit_stake`: si no entregó, desde el plazo su garantía pasa a quien publicó la recompensa.
//!
//! Traducción por encargo, la décima custodia: un trabajo largo que se paga por partes.
//! - `fund_grant`: alguien encarga la traducción de un artículo (o su resumen de divulgación) a
//!   una persona y deja en el contrato el pago completo, dividido en hitos, con un plazo.
//! - `release_milestone`: quien paga libera el siguiente hito: 98% a quien traduce y 2% a la
//!   tesorería.
//! - `cancel_grant`: pasado el plazo, quien paga recupera los hitos que no liberó.
//!
//! Agente con tope, sin custodia: un agente de IA paga lecturas con el USDC de su dueño.
//! - `authorize_agent` (de soporte, no mueve fondos): el dueño nombra a su agente, con un tope
//!   por lectura, un presupuesto total (una autorización del token al contrato) y un vencimiento.
//! - `pay_as_agent`: el agente paga una lectura con su propia firma; el dinero sale de la wallet
//!   del dueño, dentro del tope y del presupuesto, y el acceso queda a nombre del dueño.
//! - `revoke_agent` (de soporte): el dueño retira el permiso y deja la autorización en cero.
//!
//! Pagar con el saldo pendiente:
//! - `pay_from_owed`: quien tiene dinero pendiente en el contrato (una recompensa, un hito o un
//!   cobro que no pudo recibir) paga una lectura con ese saldo, sin retirarlo y sin trustline.
//!
//! Renta con opción a compra, sin custodia: leer barato por unas horas y, si sirve, comprar
//! pagando solo la diferencia.
//! - `rent`: quien lee paga la renta de un artículo por un periodo; el contrato guarda cuánto
//!   pagó, hasta cuándo y el precio de compra pactado.
//! - `buy_out`: mientras la renta siga vigente, quien lee completa la compra pagando el precio
//!   pactado menos lo que ya pagó de renta.
//!
//! Si una cuenta no puede recibir lo que le paga el contrato (por ejemplo, una editorial nueva sin
//! trustline de USDC), su parte no se pierde ni bloquea a nadie: queda pendiente a su nombre, y
//! `claim` la cobra con una firma que además le abre la trustline (`trust` del SAC, CAP-73).
//!
//! Y dos movimientos que no dan acceso (emiten su propio evento, nunca `paid`):
//! - `tip`: una propina a la editorial por un artículo, con el mismo reparto.
//! - `refund`: la editorial le devuelve dinero a un lector; firma la editorial.
//!
//! La configuración (token, tesorería, comisión y precios por día) se fija al desplegar y no
//! cambia: no hay admin ni llave que custodiar. Para cambiarla se despliega otro contrato.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, panic_with_error, token,
    xdr::ToXdr, Address, BytesN, Env, Map, String, Vec,
};

/// 100% en puntos base.
pub const BPS_DENOMINATOR: i128 = 10_000;
/// Comisión máxima aceptada al desplegar (10%): protege a la editorial de un error de captura.
pub const MAX_FEE_BPS: u32 = 1_000;
/// Largo máximo del id de un artículo, en bytes.
pub const MAX_PAPER_ID_LEN: u32 = 64;
/// Artículos por `pay_batch`: acota el costo de una transacción.
pub const MAX_BATCH_ITEMS: u32 = 10;
/// Coautores por `pay_authors`: acota el costo de una transacción.
pub const MAX_AUTHORS: u32 = 10;
/// Parte de la comisión que recibe quien recomienda en `pay_ref` (5 000 = la mitad).
pub const REFERRAL_SHARE_BPS: i128 = 5_000;
/// Días por compra de un pase (y por compra de un artículo abierto).
pub const MAX_PASS_DAYS: u32 = 31;
/// Un acceso acumulado (pase o artículo abierto) no vence más allá de esto, contado desde ahora:
/// así su entrada en la red siempre cabe en el TTL máximo (~180 días en testnet).
pub const MAX_ACCESS_DAYS: u32 = 90;
/// Segundos de un día, en la hora del ledger.
pub const DAY_SECONDS: u64 = 86_400;
/// Renovaciones por suscripción.
pub const MAX_RENEWALS: u32 = 12;
/// Días que cubre una suscripción completa (primer periodo y renovaciones): la autorización del
/// token tiene que caber en el TTL máximo de la red.
pub const MAX_SUBSCRIPTION_DAYS: u32 = 150;
/// Una renovación se puede cobrar cuando al pase le falta esto o menos.
pub const RENEW_WINDOW_SECONDS: u64 = DAY_SECONDS;
/// Plazo máximo de una campaña de acceso abierto, contado desde que se abre.
pub const MAX_CAMPAIGN_DAYS: u32 = 90;
/// Plazo máximo de una recompensa, en días.
pub const MAX_BOUNTY_DAYS: u32 = 90;
/// Vigencia máxima de un vale, en días.
pub const MAX_VOUCHER_DAYS: u32 = 90;
/// Máximo de miembros de un journal club.
pub const MAX_CLUB_MEMBERS: u32 = 10;
/// Plazo máximo de un pago contra entrega, en días: la llave se entrega al momento, no en meses.
pub const MAX_LOCK_DAYS: u32 = 7;
/// Garantía mínima para tomar una recompensa, en puntos base de la recompensa (1 000 = 10%).
pub const MIN_STAKE_BPS: i128 = 1_000;
/// Hitos por encargo: acota el costo de una transacción y el tamaño de la entrada.
pub const MAX_MILESTONES: u32 = 10;
/// Plazo máximo de un encargo, en días.
pub const MAX_GRANT_DAYS: u32 = 90;
/// Vigencia máxima del permiso de un agente, en días: un permiso de gasto se renueva seguido.
pub const MAX_AGENT_DAYS: u32 = 30;
/// Duración máxima de una renta, en segundos (7 días).
pub const MAX_RENT_SECONDS: u64 = 7 * DAY_SECONDS;
/// Máximo de editoriales con deuda en una cuenta abierta.
pub const MAX_TAB_PUBLISHERS: u32 = 10;
/// Después del plazo, las entradas de una campaña siguen en la red al menos estos días, para que
/// cada aporte de una campaña fallida se pueda devolver.
pub const REFUND_GRACE_DAYS: u32 = 30;

// La instancia vive mientras haya pagos: cada cobro extiende su TTL a ~30 días.
const DAY_IN_LEDGERS: u32 = 17_280;
const INSTANCE_TTL_THRESHOLD: u32 = 7 * DAY_IN_LEDGERS;
const INSTANCE_TTL_EXTEND_TO: u32 = 30 * DAY_IN_LEDGERS;
// Un ledger cada ~5 s. Las entradas con vencimiento viven hasta él y una semana más.
const LEDGER_SECONDS: u64 = 5;
/// Cuánto dura el marcador de una lectura patrocinada: un día de ledgers (~24 h).
const SPONSORED_TTL_LEDGERS: u32 = DAY_IN_LEDGERS;
const TTL_MARGIN_LEDGERS: u32 = 7 * DAY_IN_LEDGERS;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// `fee_bps` mayor que `MAX_FEE_BPS`.
    FeeTooHigh = 1,
    /// Monto menor o igual a cero.
    InvalidAmount = 2,
    /// Id de artículo vacío o de más de `MAX_PAPER_ID_LEN` bytes.
    InvalidPaperId = 3,
    /// Un monto (o la suma de un paquete) es tan grande que el cálculo se desborda.
    AmountOverflow = 4,
    /// Una lista vacía: `pay_batch` sin artículos o una compra o devolución de vales sin llaves.
    EmptyBatch = 5,
    /// Una lista de más de `MAX_BATCH_ITEMS`: artículos en `pay_batch` o llaves de vales.
    BatchTooLarge = 6,
    /// `pay_batch` con el mismo artículo dos veces.
    DuplicatePaper = 7,
    /// `pay_authors` con partes inválidas: lista vacía, una parte en 0 o de más del 100%,
    /// partes que no suman 100% o un autor repetido.
    InvalidShares = 8,
    /// `pay_authors` con más de `MAX_AUTHORS` coautores.
    TooManyAuthors = 9,
    /// `pay_ref` con `referrer` igual a `payer`: sería un descuento, no una recomendación.
    SelfReferral = 10,
    /// Días fuera de rango: 0 o más de `MAX_PASS_DAYS` (un pase, un artículo abierto, o el plazo
    /// de pago de una cuenta abierta y la duración de un Pase PaperPay, que van en segundos).
    InvalidDays = 11,
    /// `amount` distinto de días × precio por día de la configuración.
    WrongPrice = 12,
    /// El acceso acumulado vencería después de `MAX_ACCESS_DAYS`, o la suscripción cubre más
    /// de `MAX_SUBSCRIPTION_DAYS`.
    TooLong = 13,
    /// Renovaciones fuera de rango: 0 o más de `MAX_RENEWALS`.
    InvalidRenewals = 14,
    /// `reader` no tiene suscripción (nunca tuvo, la canceló o ya cobró todas las renovaciones).
    NoSubscription = 15,
    /// Al pase le falta más de `RENEW_WINDOW_SECONDS`: todavía no toca renovar.
    RenewTooEarly = 16,
    /// Plazo que ya pasó o que queda demasiado lejos: más de `MAX_CAMPAIGN_DAYS` para una campaña,
    /// de `MAX_BOUNTY_DAYS` para una recompensa o de `MAX_VOUCHER_DAYS` para un vale.
    InvalidDeadline = 17,
    /// No hay campaña con ese id.
    NoCampaign = 18,
    /// La campaña ya no recibe aportes (venció, llegó a la meta o se liberó), o ya se liberó.
    CampaignClosed = 19,
    /// El aporte supera lo que falta para la meta. El monto que se firma no se recorta: si otro
    /// aporte entra antes, este falla sin mover nada y se vuelve a intentar con lo que falte.
    OverGoal = 20,
    /// Todavía no se llega a la meta de la campaña.
    GoalNotReached = 21,
    /// La cuenta no tiene nada pendiente de cobro.
    NothingToClaim = 22,
    /// La campaña no falló: todavía no vence, llegó a la meta o ya se liberó.
    CampaignNotFailed = 23,
    /// Esa cuenta no tiene aporte en garantía en la campaña (nunca aportó o ya se le devolvió).
    NothingToRefund = 24,
    /// Ese patrocinador no tiene fondo para esa editorial.
    NoPool = 25,
    /// Quien firma no es el operador del fondo, de la cuenta abierta o del Pase PaperPay.
    NotOperator = 26,
    /// La lectura cuesta más que el tope por lectura del fondo.
    AboveMaxPerRead = 27,
    /// Al fondo no le alcanza para esa lectura o para ese retiro.
    PoolTooLow = 28,
    /// El fondo ya pagó ese artículo para ese lector en las últimas 24 h.
    AlreadySponsored = 29,
    /// No hay recompensa con ese id publicada por esa cuenta.
    NoBounty = 30,
    /// La recompensa ya se pagó o se canceló.
    BountyClosed = 31,
    /// La recompensa sigue en pie: no se cancela antes de su plazo.
    BountyActive = 32,
    /// No hay vale con esa llave (o, al devolver, no es de esa cuenta).
    NoVoucher = 33,
    /// El vale ya se canjeó o se devolvió.
    VoucherSpent = 34,
    /// El vale venció: ya no se canjea.
    VoucherExpired = 35,
    /// El vale sigue vigente: no se devuelve antes de vencer.
    VoucherActive = 36,
    /// Esa llave ya se usó para un vale (también si está repetida en la misma compra). Una llave
    /// no se reutiliza nunca: así una firma vieja no sirve para un vale nuevo.
    VoucherExists = 37,
    /// Esa cuenta no tiene cuenta abierta.
    NoTab = 38,
    /// Esa cuenta ya tiene una cuenta abierta: se cierra antes de abrir otra.
    TabExists = 39,
    /// La lectura no cabe: la deuda pasaría de la garantía, o ya hay `MAX_TAB_PUBLISHERS`
    /// editoriales con deuda.
    TabFull = 40,
    /// La cuenta abierta no tiene deuda: no hay nada que pagar ni que cobrar.
    NothingOwed = 41,
    /// Todavía no vence el plazo de pago: la garantía no se toca.
    TabNotDue = 42,
    /// La cuenta abierta tiene deuda: no se cierra hasta pagarla.
    TabOwes = 43,
    /// Esa cuenta no tiene Pase PaperPay.
    NoGlobalPass = 44,
    /// Esa cuenta ya tiene un Pase PaperPay: se liquida o se cancela antes de comprar otro.
    PassExists = 45,
    /// El Pase PaperPay ya venció: no se le anotan más lecturas.
    PassExpired = 46,
    /// El Pase PaperPay ya tiene lecturas de `MAX_TAB_PUBLISHERS` editoriales.
    PassFull = 47,
    /// El Pase PaperPay sigue vigente: no se liquida antes de vencer.
    PassActive = 48,
    /// El Pase PaperPay ya tiene lecturas: no se cancela, se liquida al vencer.
    PassUsed = 49,
    /// No hay journal club con ese id.
    NoClub = 50,
    /// Quórum fuera de rango: menos de 2 o más de `MAX_CLUB_MEMBERS`.
    InvalidQuorum = 51,
    /// El club ya tiene `MAX_CLUB_MEMBERS` miembros.
    ClubFull = 52,
    /// Esa cuenta ya es miembro del club.
    AlreadyMember = 53,
    /// Esa cuenta no es miembro del club.
    NotMember = 54,
    /// A la caja del club no le alcanza para esa lectura.
    ClubTooLow = 55,
    /// No hay propuesta con ese id en el club.
    NoProposal = 56,
    /// La propuesta ya se pagó.
    ProposalClosed = 57,
    /// Ese miembro ya aprobó la propuesta.
    AlreadyApproved = 58,
    /// No hay pago contra entrega con ese id (o no es de esa cuenta).
    NoLock = 59,
    /// El pago contra entrega ya se cobró o se devolvió.
    LockClosed = 60,
    /// El sha256 de esa llave no es el que se pactó al bloquear el pago.
    WrongKey = 61,
    /// El plazo de entrega ya venció: el pago solo puede volver a quien lo hizo.
    LockExpired = 62,
    /// El plazo de entrega sigue corriendo: todavía no se puede recuperar el pago.
    LockActive = 63,
    /// El plazo de la recompensa ya venció: no se puede tomar ni entregar.
    BountyExpired = 64,
    /// La garantía es menor al mínimo (el 10% de la recompensa).
    StakeTooLow = 65,
    /// Esa cuenta ya tomó la recompensa y su garantía sigue en el contrato.
    AlreadyAccepted = 66,
    /// Esa cuenta no tiene garantía en esa recompensa: no la tomó, ya entregó o ya la perdió.
    NoStake = 67,
    /// Los hitos de un encargo: de 1 a `MAX_MILESTONES`, cada uno mayor que cero.
    InvalidMilestones = 68,
    /// No hay encargo con ese id (o no es de esa cuenta).
    NoGrant = 69,
    /// El encargo ya se pagó completo o se canceló.
    GrantClosed = 70,
    /// El plazo del encargo sigue corriendo: todavía no se puede cancelar.
    GrantActive = 71,
    /// Esa cuenta no es el agente autorizado de ese dueño (o el dueño no autorizó a ninguno).
    NoAgent = 72,
    /// El permiso del agente ya venció.
    AgentExpired = 73,
    /// Al presupuesto del agente no le alcanza para ese pago.
    BudgetExhausted = 74,
    /// La autorización del token ya es de la suscripción de esa cuenta: no se comparte con un
    /// agente.
    AllowanceInUse = 75,
    /// A lo pendiente de esa cuenta no le alcanza para ese pago.
    OwedTooLow = 76,
    /// Esa cuenta no tiene una renta de ese artículo (o ya la convirtió en compra).
    NoRental = 77,
    /// La renta ya venció: la compra con descuento ya no aplica.
    RentalExpired = 78,
    /// Esa cuenta ya tiene una renta vigente de ese artículo.
    RentalActive = 79,
    /// La renta tiene que costar menos que la compra.
    RentNotCheaper = 80,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    /// Contrato del token (el SAC de USDC).
    pub token: Address,
    /// Cuenta que recibe la comisión de PaperPay.
    pub treasury: Address,
    /// Comisión en puntos base (200 = 2%).
    pub fee_bps: u32,
    /// Precio de un día de pase al catálogo de una editorial, en unidades del token.
    pub pass_day_price: i128,
    /// Precio de un día de acceso abierto (para todos) a un artículo, en unidades del token.
    pub open_day_price: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Split {
    /// Lo que reciben las editoriales.
    pub publisher: i128,
    /// Lo que recibe la tesorería.
    pub fee: i128,
}

/// Reparto de `pay_ref`: la editorial recibe lo mismo que en `pay`; la comisión se parte entre
/// la tesorería (`fee`) y quien recomendó (`referral`).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefSplit {
    pub publisher: i128,
    pub fee: i128,
    pub referral: i128,
}

/// Un artículo dentro de `pay_batch`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Item {
    pub paper_id: String,
    pub publisher: Address,
    pub amount: i128,
}

/// Un coautor dentro de `pay_authors`: su cuenta y su parte de lo que le toca a la editorial,
/// en puntos base (2 500 = 25%).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Share {
    pub account: Address,
    pub bps: u32,
}

/// La suscripción de un lector: cada renovación compra `days` días de pase al catálogo de
/// `publisher` por `amount` (días × precio por día). Quedan `renewals_left` renovaciones y la
/// autorización para cobrarlas vence en el ledger `allowance_until`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Subscription {
    pub publisher: Address,
    pub days: u32,
    pub amount: i128,
    pub renewals_left: u32,
    pub allowance_until: u32,
}

/// Una campaña de acceso abierto: `publisher` se compromete a liberar `paper_id` para todos si
/// los aportes llegan a `goal` antes de `deadline` (segundos Unix, hora del ledger). `raised` es lo
/// aportado hasta ahora y `released`, si ya se liberó.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Campaign {
    pub publisher: Address,
    pub paper_id: String,
    pub goal: i128,
    pub deadline: u64,
    pub raised: i128,
    pub released: bool,
}

/// Un fondo de patrocinio: lo que un patrocinador dejó en el contrato para pagar lecturas de una
/// editorial. `operator` es la única cuenta que puede gastarlo (`pay_sponsored`), hasta
/// `max_per_read` por lectura. `balance` es lo que queda.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pool {
    pub operator: Address,
    pub max_per_read: i128,
    pub balance: i128,
}

/// Una recompensa por revisar o reproducir `paper_id`: `funder` dejó `amount` en el contrato
/// hasta `deadline` (segundos Unix, hora del ledger). `open` es falso cuando ya se pagó o se canceló.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bounty {
    pub funder: Address,
    pub paper_id: String,
    pub amount: i128,
    pub deadline: u64,
    pub open: bool,
}

/// Un vale: una lectura de `value` del catálogo de `publisher`, pagada por `buyer` y vigente
/// hasta `expires_at` (segundos Unix). `open` es falso cuando ya se canjeó o se devolvió.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Voucher {
    pub buyer: Address,
    pub publisher: Address,
    pub value: i128,
    pub expires_at: u64,
    pub open: bool,
}

/// Una cuenta abierta: quien lee dejó `deposit` en el contrato como garantía y `operator` anota sus
/// lecturas en `debts` (lo que se le debe a cada editorial). `owed` es la suma, que nunca pasa de
/// `deposit`. Desde la primera lectura sin pagar hay `period` segundos para pagar: `due` (segundos
/// Unix) es cuándo vence ese plazo, o 0 si no hay deuda.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tab {
    pub operator: Address,
    pub deposit: i128,
    pub period: u64,
    pub owed: i128,
    pub due: u64,
    pub debts: Map<Address, i128>,
}

/// Un Pase PaperPay: `reader` pagó `amount` por leer todo el catálogo hasta `until` (segundos
/// Unix). `operator` anota sus lecturas: `reads` es el total y `by_publisher`, cuántas de cada
/// editorial. Al vencer, `amount` se reparte en esa proporción.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GlobalPass {
    pub operator: Address,
    pub amount: i128,
    pub until: u64,
    pub reads: u32,
    pub by_publisher: Map<Address, u32>,
}

/// Un journal club: cada miembro pagó `dues` a la caja (`balance`). Una lectura se paga cuando la
/// aprueban `quorum` miembros. `proposals` es cuántas propuestas se han hecho (los ids van de 0).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Club {
    pub dues: i128,
    pub quorum: u32,
    pub members: Vec<Address>,
    pub balance: i128,
    pub proposals: u32,
}

/// Una propuesta de lectura de un club: pagar `amount` a `publisher` por `paper_id`. `approvals`
/// son los miembros que la aprobaron (quien la propuso es el primero) y `paid`, si ya se pagó.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    pub publisher: Address,
    pub paper_id: String,
    pub amount: i128,
    pub approvals: Vec<Address>,
    pub paid: bool,
}

/// Un pago contra entrega: `reader` dejó `amount` en el contrato por `paper_id` de `publisher`.
/// `key_hash` es el sha256 de la llave que abre el artículo: quien la presente antes de `deadline`
/// (segundos Unix) libera el pago a la editorial. `open` es falso cuando ya se cobró o se devolvió.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lock {
    pub reader: Address,
    pub publisher: Address,
    pub paper_id: String,
    pub amount: i128,
    pub key_hash: BytesN<32>,
    pub deadline: u64,
    pub open: bool,
}

/// Un encargo de traducción de `paper_id`: `funder` dejó en el contrato la suma de `milestones`
/// para `translator`. Los hitos se liberan en orden y `released` es cuántos se pagaron. Hasta
/// `deadline` (segundos Unix) el dinero solo puede salir hacia `translator`; después, lo no
/// liberado puede volver a `funder`. `open` es falso cuando se pagaron todos o se canceló.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Grant {
    pub funder: Address,
    pub translator: Address,
    pub paper_id: String,
    pub milestones: Vec<i128>,
    pub released: u32,
    pub deadline: u64,
    pub open: bool,
}

/// El permiso de un agente: `agent` puede pagar lecturas con el USDC de su dueño, hasta
/// `max_per_read` por lectura, mientras no llegue `expires_at` (segundos Unix). El presupuesto
/// total no se guarda aquí: es la autorización (allowance) que el dueño le dio al contrato en el
/// token, que baja con cada pago.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentPermit {
    pub agent: Address,
    pub max_per_read: i128,
    pub expires_at: u64,
}

/// Una renta: `paid` es lo que ya se pagó por leer el artículo hasta `until` (segundos Unix) y
/// `full_price`, lo que cuesta comprarlo. Mientras la renta siga vigente, la compra cuesta solo
/// la diferencia.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rental {
    pub paid: i128,
    pub full_price: i128,
    pub until: u64,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Config,
    /// Vencimiento (segundos Unix) del pase de `reader` al catálogo de `publisher`.
    Pass(Address, Address),
    /// La suscripción de `reader` (una por lector: la allowance del token es una por cuenta).
    Sub(Address),
    /// Hasta cuándo (segundos Unix) el artículo `paper_id` de `publisher` está abierto para todos.
    Open(Address, String),
    /// Id de la próxima campaña (instance): los ids son secuenciales desde 0.
    NextCampaign,
    /// Una campaña de acceso abierto, por id.
    Campaign(u32),
    /// Lo que aportó `backer` a la campaña `u32` y sigue en garantía.
    Pledge(u32, Address),
    /// Lo que el contrato le debe a una cuenta que no pudo recibir un pago.
    Owed(Address),
    /// El fondo de patrocinio de `sponsor` para las lecturas de `publisher`.
    Pool(Address, Address),
    /// Marcador temporal: el fondo de `sponsor` ya pagó `paper_id` para `reader`. Vence solo en
    /// `SPONSORED_TTL_LEDGERS`.
    Sponsored(Address, Address, String),
    /// Id de la próxima recompensa (instance): los ids son secuenciales desde 0.
    NextBounty,
    /// Una recompensa, por id.
    Bounty(u32),
    /// Un vale, por su llave pública ed25519.
    Voucher(BytesN<32>),
    /// La cuenta abierta de un lector (una por lector).
    Tab(Address),
    /// El Pase PaperPay de un lector (uno a la vez).
    GlobalPass(Address),
    /// Id del próximo journal club (instance): los ids son secuenciales desde 0.
    NextClub,
    /// Un journal club, por id.
    Club(u32),
    /// La propuesta `.1` del club `.0`.
    Proposal(u32, u32),
    /// Id del próximo pago contra entrega (instance): los ids son secuenciales desde 0.
    NextLock,
    /// Un pago contra entrega, por id.
    Lock(u32),
    /// La garantía que dejó un revisor al tomar la recompensa `u32`.
    Stake(u32, Address),
    /// Id del próximo encargo (instance): los ids son secuenciales desde 0.
    NextGrant,
    /// Un encargo de traducción, por id.
    Grant(u32),
    /// El permiso del agente de un dueño (uno por dueño: la allowance del token es una por cuenta).
    Agent(Address),
    /// La renta de `reader` (`.0`) del artículo `.2` de `publisher` (`.1`).
    Rental(Address, Address, String),
}

/// Comprobante de un artículo pagado. Topics: `["paid", publisher, reader]`.
/// `reader` es quien obtiene el acceso: `payer` en `pay` y `pay_batch`, otra cuenta en `gift`.
/// En `pay_authors`, `publisher` es el primer autor de la lista (el de correspondencia).
#[contractevent(topics = ["paid"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Paid {
    #[topic]
    pub publisher: Address,
    #[topic]
    pub reader: Address,
    pub paper_id: String,
    pub payer: Address,
    pub amount: i128,
    pub fee: i128,
}

/// Recompensa por recomendar una lectura. Topics: `["referred", referrer, payer]`. Acompaña al
/// `paid` de la misma lectura; `amount` es el precio del artículo y `reward` lo que recibió
/// `referrer` (sale de la comisión).
#[contractevent(topics = ["referred"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Referred {
    #[topic]
    pub referrer: Address,
    #[topic]
    pub payer: Address,
    pub paper_id: String,
    pub publisher: Address,
    pub amount: i128,
    pub reward: i128,
}

/// Comprobante de un pase. Topics: `["pass", publisher, reader]`. `reader` puede leer todo el
/// catálogo de `publisher` hasta `valid_until` (segundos Unix, hora del ledger). `amount` es lo que
/// se cobró por `days` días y `fee` la comisión.
#[contractevent(topics = ["pass"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PassBought {
    #[topic]
    pub publisher: Address,
    #[topic]
    pub reader: Address,
    pub payer: Address,
    pub days: u32,
    pub amount: i128,
    pub fee: i128,
    pub valid_until: u64,
}

/// Alta de una suscripción. Topics: `["subscribed", publisher, reader]`. Acompaña al `pass` del
/// primer periodo. `renewals` es cuántas renovaciones autorizó `reader`, por `amount` cada una.
#[contractevent(topics = ["subscribed"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Subscribed {
    #[topic]
    pub publisher: Address,
    #[topic]
    pub reader: Address,
    pub days: u32,
    pub amount: i128,
    pub renewals: u32,
    pub allowance_until: u32,
}

/// Una renovación cobrada. Topics: `["renewed", publisher, reader]`. Acompaña al `pass` que
/// extiende el acceso.
#[contractevent(topics = ["renewed"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Renewed {
    #[topic]
    pub publisher: Address,
    #[topic]
    pub reader: Address,
    pub days: u32,
    pub amount: i128,
    pub fee: i128,
    pub valid_until: u64,
    pub renewals_left: u32,
}

/// Una suscripción cancelada. Topics: `["unsubscribed", publisher, reader]`.
#[contractevent(topics = ["unsubscribed"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unsubscribed {
    #[topic]
    pub publisher: Address,
    #[topic]
    pub reader: Address,
    pub renewals_left: u32,
}

/// Un artículo abierto para todos por un patrocinio. Topics: `["opened", publisher, sponsor]`.
/// Cualquiera puede leer `paper_id` hasta `open_until` (segundos Unix, hora del ledger).
#[contractevent(topics = ["opened"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Opened {
    #[topic]
    pub publisher: Address,
    #[topic]
    pub sponsor: Address,
    pub paper_id: String,
    pub days: u32,
    pub amount: i128,
    pub fee: i128,
    pub open_until: u64,
}

/// Una campaña nueva. Topics: `["campaign", publisher]`. Es la oferta pública de la editorial:
/// liberar `paper_id` para todos si los aportes llegan a `goal` antes de `deadline`.
#[contractevent(topics = ["campaign"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CampaignOpened {
    #[topic]
    pub publisher: Address,
    pub campaign_id: u32,
    pub paper_id: String,
    pub goal: i128,
    pub deadline: u64,
}

/// Un aporte en garantía. Topics: `["pledged", backer]`. `raised` es lo aportado a la campaña
/// después de este aporte.
#[contractevent(topics = ["pledged"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pledged {
    #[topic]
    pub backer: Address,
    pub campaign_id: u32,
    pub amount: i128,
    pub raised: i128,
}

/// Una campaña cumplida. Topics: `["released", publisher]`. `paper_id` queda liberado para todos;
/// `raised` es lo que se juntó y `fee` la comisión. Es distinto de `opened` (`sponsor_open`), que
/// abre un artículo por días.
#[contractevent(topics = ["released"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Released {
    #[topic]
    pub publisher: Address,
    pub campaign_id: u32,
    pub paper_id: String,
    pub raised: i128,
    pub fee: i128,
}

/// Un pago del contrato que la cuenta no pudo recibir y quedó pendiente a su nombre.
/// Topics: `["credited", account]`.
#[contractevent(topics = ["credited"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Credited {
    #[topic]
    pub account: Address,
    pub amount: i128,
}

/// Un aporte devuelto porque la campaña falló. Topics: `["unpledged", backer]`.
#[contractevent(topics = ["unpledged"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unpledged {
    #[topic]
    pub backer: Address,
    pub campaign_id: u32,
    pub amount: i128,
}

/// Un cobro de lo pendiente. Topics: `["claimed", account]`.
#[contractevent(topics = ["claimed"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Claimed {
    #[topic]
    pub account: Address,
    pub amount: i128,
}

/// Comprobante de una propina. Topics: `["tipped", publisher, payer]`. No da acceso al artículo.
#[contractevent(topics = ["tipped"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tipped {
    #[topic]
    pub publisher: Address,
    #[topic]
    pub payer: Address,
    pub paper_id: String,
    pub amount: i128,
    pub fee: i128,
}

/// Comprobante de una devolución. Topics: `["refunded", publisher, reader]`.
#[contractevent(topics = ["refunded"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Refunded {
    #[topic]
    pub publisher: Address,
    #[topic]
    pub reader: Address,
    pub paper_id: String,
    pub amount: i128,
}

/// Un depósito a un fondo de patrocinio. Topics: `["pool_funded", sponsor, publisher]`.
/// `balance` es el saldo del fondo después del depósito.
#[contractevent(topics = ["pool_funded"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolFunded {
    #[topic]
    pub sponsor: Address,
    #[topic]
    pub publisher: Address,
    pub operator: Address,
    pub max_per_read: i128,
    pub amount: i128,
    pub balance: i128,
}

/// Una lectura pagada por un fondo. Topics: `["sponsored", sponsor, reader]`. Acompaña al `paid`
/// de la misma lectura; `balance` es lo que le queda al fondo.
#[contractevent(topics = ["sponsored"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SponsoredRead {
    #[topic]
    pub sponsor: Address,
    #[topic]
    pub reader: Address,
    pub paper_id: String,
    pub publisher: Address,
    pub amount: i128,
    pub balance: i128,
}

/// Un retiro de un fondo de patrocinio. Topics: `["pool_withdrawn", sponsor, publisher]`.
#[contractevent(topics = ["pool_withdrawn"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolWithdrawn {
    #[topic]
    pub sponsor: Address,
    #[topic]
    pub publisher: Address,
    pub amount: i128,
    pub balance: i128,
}

/// Una recompensa nueva. Topics: `["bounty_posted", funder]`. `amount` ya está en el contrato.
#[contractevent(topics = ["bounty_posted"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BountyPosted {
    #[topic]
    pub funder: Address,
    pub bounty_id: u32,
    pub paper_id: String,
    pub amount: i128,
    pub deadline: u64,
}

/// Una recompensa repartida. Topics: `["bounty_awarded", funder]`. `amount` es el total y `fee` la
/// comisión; lo que recibió cada ganador va en su propio evento `rewarded`.
#[contractevent(topics = ["bounty_awarded"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BountyAwarded {
    #[topic]
    pub funder: Address,
    pub bounty_id: u32,
    pub paper_id: String,
    pub amount: i128,
    pub fee: i128,
}

/// La parte de una recompensa que le tocó a un ganador. Topics: `["rewarded", winner]`.
#[contractevent(topics = ["rewarded"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rewarded {
    #[topic]
    pub winner: Address,
    pub bounty_id: u32,
    pub amount: i128,
}

/// Una recompensa cancelada después de su plazo. Topics: `["bounty_cancelled", funder]`.
#[contractevent(topics = ["bounty_cancelled"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BountyCancelled {
    #[topic]
    pub funder: Address,
    pub bounty_id: u32,
    pub amount: i128,
}

/// Una compra de vales. Topics: `["vouchers_bought", buyer, publisher]`. `total` (el valor de cada
/// vale por la cantidad de llaves) ya está en el contrato.
#[contractevent(topics = ["vouchers_bought"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VouchersBought {
    #[topic]
    pub buyer: Address,
    #[topic]
    pub publisher: Address,
    pub keys: Vec<BytesN<32>>,
    pub value: i128,
    pub expires_at: u64,
    pub total: i128,
}

/// Un vale canjeado. Topics: `["voucher_redeemed", buyer, reader]`. Acompaña al `paid` de la
/// lectura.
#[contractevent(topics = ["voucher_redeemed"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoucherRedeemed {
    #[topic]
    pub buyer: Address,
    #[topic]
    pub reader: Address,
    pub key: BytesN<32>,
    pub paper_id: String,
    pub value: i128,
}

/// Vales vencidos devueltos a quien los compró. Topics: `["vouchers_reclaimed", buyer]`.
#[contractevent(topics = ["vouchers_reclaimed"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VouchersReclaimed {
    #[topic]
    pub buyer: Address,
    pub keys: Vec<BytesN<32>>,
    pub total: i128,
}

/// Una cuenta abierta nueva. Topics: `["tab_opened", reader]`. `deposit` ya está en el contrato.
#[contractevent(topics = ["tab_opened"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabOpened {
    #[topic]
    pub reader: Address,
    pub operator: Address,
    pub deposit: i128,
    pub period: u64,
}

/// Una lectura anotada en una cuenta abierta. Topics: `["charged", reader, publisher]`. No movió
/// fondos: `owed` es la deuda total después de anotarla y `due`, cuándo vence el plazo de pago.
#[contractevent(topics = ["charged"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Charged {
    #[topic]
    pub reader: Address,
    #[topic]
    pub publisher: Address,
    pub paper_id: String,
    pub amount: i128,
    pub owed: i128,
    pub due: u64,
}

/// Una cuenta abierta pagada por quien lee. Topics: `["tab_settled", reader]`. `total` es toda la
/// deuda, `fee` la comisión y `publishers` cuántas editoriales cobraron.
#[contractevent(topics = ["tab_settled"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabSettled {
    #[topic]
    pub reader: Address,
    pub total: i128,
    pub fee: i128,
    pub publishers: u32,
}

/// Una deuda vencida cobrada de la garantía. Topics: `["tab_collected", reader]`. `deposit` es la
/// garantía que queda.
#[contractevent(topics = ["tab_collected"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabCollected {
    #[topic]
    pub reader: Address,
    pub total: i128,
    pub fee: i128,
    pub publishers: u32,
    pub deposit: i128,
}

/// Una cuenta abierta cerrada. Topics: `["tab_closed", reader]`. `amount` es la garantía devuelta.
#[contractevent(topics = ["tab_closed"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabClosed {
    #[topic]
    pub reader: Address,
    pub amount: i128,
}

/// Un Pase PaperPay nuevo. Topics: `["global_pass", reader]`. `amount` ya está en el contrato.
#[contractevent(topics = ["global_pass"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GlobalPassBought {
    #[topic]
    pub reader: Address,
    pub operator: Address,
    pub amount: i128,
    pub until: u64,
}

/// Una lectura hecha con un Pase PaperPay. Topics: `["pass_read", reader, publisher]`. No movió
/// fondos: `reads` es el total de lecturas del pase después de anotarla.
#[contractevent(topics = ["pass_read"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PassRead {
    #[topic]
    pub reader: Address,
    #[topic]
    pub publisher: Address,
    pub paper_id: String,
    pub reads: u32,
}

/// Lo que le tocó a una editorial al liquidarse un Pase PaperPay. Topics:
/// `["pass_share", publisher, reader]`. `reads` son las lecturas de esa editorial.
#[contractevent(topics = ["pass_share"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PassShare {
    #[topic]
    pub publisher: Address,
    #[topic]
    pub reader: Address,
    pub reads: u32,
    pub amount: i128,
}

/// Un Pase PaperPay liquidado. Topics: `["pass_settled", reader]`. `amount` es lo que costó;
/// con lecturas, `fee` fue a la tesorería y el resto a `publishers` editoriales. Sin lecturas
/// (`reads` = 0), todo volvió a `reader` y `fee` es 0.
#[contractevent(topics = ["pass_settled"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PassSettled {
    #[topic]
    pub reader: Address,
    pub amount: i128,
    pub fee: i128,
    pub reads: u32,
    pub publishers: u32,
}

/// Un Pase PaperPay cancelado sin lecturas. Topics: `["pass_cancelled", reader]`.
#[contractevent(topics = ["pass_cancelled"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PassCancelled {
    #[topic]
    pub reader: Address,
    pub amount: i128,
}

/// Un journal club nuevo. Topics: `["club_created", founder]`. No movió fondos.
#[contractevent(topics = ["club_created"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClubCreated {
    #[topic]
    pub founder: Address,
    pub club_id: u32,
    pub dues: i128,
    pub quorum: u32,
}

/// Un miembro nuevo. Topics: `["club_joined", member]`. Su cuota ya está en la caja: `balance` es
/// la caja después y `members`, cuántos miembros hay.
#[contractevent(topics = ["club_joined"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClubJoined {
    #[topic]
    pub member: Address,
    pub club_id: u32,
    pub dues: i128,
    pub balance: i128,
    pub members: u32,
}

/// Una propuesta de lectura. Topics: `["read_proposed", member]`. No movió fondos.
#[contractevent(topics = ["read_proposed"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadProposed {
    #[topic]
    pub member: Address,
    pub club_id: u32,
    pub proposal_id: u32,
    pub publisher: Address,
    pub paper_id: String,
    pub amount: i128,
}

/// Una aprobación. Topics: `["read_approved", member]`. `approvals` cuenta las de miembros
/// actuales y `paid` dice si esta aprobación completó el quórum y pagó.
#[contractevent(topics = ["read_approved"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadApproved {
    #[topic]
    pub member: Address,
    pub club_id: u32,
    pub proposal_id: u32,
    pub approvals: u32,
    pub paid: bool,
}

/// Una lectura pagada por un club. Topics: `["club_paid", publisher]`. `amount` salió de la caja
/// (`fee` es la comisión) y el artículo es para todos los `members` del club en ese momento.
#[contractevent(topics = ["club_paid"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClubPaid {
    #[topic]
    pub publisher: Address,
    pub club_id: u32,
    pub proposal_id: u32,
    pub paper_id: String,
    pub amount: i128,
    pub fee: i128,
    pub members: Vec<Address>,
}

/// Un miembro que sale. Topics: `["club_left", member]`. `amount` es lo que se llevó y `balance`,
/// lo que queda en la caja.
#[contractevent(topics = ["club_left"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClubLeft {
    #[topic]
    pub member: Address,
    pub club_id: u32,
    pub amount: i128,
    pub balance: i128,
}

/// Un pago bloqueado hasta la entrega. Topics: `["payment_locked", reader, publisher]`. `amount`
/// ya está en el contrato.
#[contractevent(topics = ["payment_locked"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentLocked {
    #[topic]
    pub reader: Address,
    #[topic]
    pub publisher: Address,
    pub lock_id: u32,
    pub paper_id: String,
    pub amount: i128,
    pub key_hash: BytesN<32>,
    pub deadline: u64,
}

/// Un pago liberado con la llave. Topics: `["payment_unlocked", reader, publisher]`. `key` es la
/// llave que abre el artículo: queda publicada aquí, y así la recibe quien pagó. Acompaña al
/// `paid` de la lectura.
#[contractevent(topics = ["payment_unlocked"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentUnlocked {
    #[topic]
    pub reader: Address,
    #[topic]
    pub publisher: Address,
    pub lock_id: u32,
    pub key: BytesN<32>,
}

/// Un pago devuelto porque la llave no llegó a tiempo. Topics: `["payment_reclaimed", reader]`.
#[contractevent(topics = ["payment_reclaimed"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentReclaimed {
    #[topic]
    pub reader: Address,
    pub lock_id: u32,
    pub amount: i128,
}

/// Una recompensa tomada. Topics: `["bounty_accepted", reviewer]`. `stake` ya está en el contrato
/// y `deadline` es hasta cuándo hay que entregar.
#[contractevent(topics = ["bounty_accepted"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BountyAccepted {
    #[topic]
    pub reviewer: Address,
    pub bounty_id: u32,
    pub stake: i128,
    pub deadline: u64,
}

/// Una entrega a tiempo. Topics: `["review_submitted", reviewer]`. `report_hash` es el sha256 del
/// informe y `stake`, la garantía que volvió a quien entregó.
#[contractevent(topics = ["review_submitted"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewSubmitted {
    #[topic]
    pub reviewer: Address,
    pub bounty_id: u32,
    pub report_hash: BytesN<32>,
    pub stake: i128,
}

/// Una garantía perdida por no entregar. Topics: `["stake_forfeited", reviewer, funder]`.
/// `amount` pasó a `funder`, quien publicó la recompensa.
#[contractevent(topics = ["stake_forfeited"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StakeForfeited {
    #[topic]
    pub reviewer: Address,
    #[topic]
    pub funder: Address,
    pub bounty_id: u32,
    pub amount: i128,
}

/// Un encargo nuevo. Topics: `["grant_funded", funder, translator]`. `total` (la suma de los
/// hitos) ya está en el contrato y `milestones` es cuántos hitos son.
#[contractevent(topics = ["grant_funded"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrantFunded {
    #[topic]
    pub funder: Address,
    #[topic]
    pub translator: Address,
    pub grant_id: u32,
    pub paper_id: String,
    pub total: i128,
    pub milestones: u32,
    pub deadline: u64,
}

/// Un hito pagado. Topics: `["milestone_released", translator, funder]`. `milestone` es su
/// posición (desde 0), `amount` el hito completo, `fee` la comisión y `remaining`, cuántos faltan.
#[contractevent(topics = ["milestone_released"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MilestoneReleased {
    #[topic]
    pub translator: Address,
    #[topic]
    pub funder: Address,
    pub grant_id: u32,
    pub milestone: u32,
    pub amount: i128,
    pub fee: i128,
    pub remaining: u32,
}

/// Un encargo cancelado después de su plazo. Topics: `["grant_cancelled", funder]`. `amount` es
/// la suma de los hitos no liberados, que volvió a `funder`.
#[contractevent(topics = ["grant_cancelled"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrantCancelled {
    #[topic]
    pub funder: Address,
    pub grant_id: u32,
    pub amount: i128,
}

/// Un agente autorizado. Topics: `["agent_authorized", owner, agent]`. No se movió nada: `budget`
/// es lo que el contrato puede cobrarle a `owner` por los pagos de su agente.
#[contractevent(topics = ["agent_authorized"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentAuthorized {
    #[topic]
    pub owner: Address,
    #[topic]
    pub agent: Address,
    pub max_per_read: i128,
    pub budget: i128,
    pub expires_at: u64,
}

/// Una lectura pagada por un agente. Topics: `["agent_paid", owner, agent]`. Acompaña al `paid`
/// de la lectura; `remaining` es el presupuesto que queda.
#[contractevent(topics = ["agent_paid"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentPaid {
    #[topic]
    pub owner: Address,
    #[topic]
    pub agent: Address,
    pub paper_id: String,
    pub amount: i128,
    pub remaining: i128,
}

/// Un permiso retirado. Topics: `["agent_revoked", owner, agent]`.
#[contractevent(topics = ["agent_revoked"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRevoked {
    #[topic]
    pub owner: Address,
    #[topic]
    pub agent: Address,
}

/// Una lectura pagada con lo pendiente. Topics: `["paid_from_owed", account]`. Acompaña al `paid`
/// de la lectura; `remaining` es lo que el contrato le sigue debiendo a `account`.
#[contractevent(topics = ["paid_from_owed"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaidFromOwed {
    #[topic]
    pub account: Address,
    pub paper_id: String,
    pub amount: i128,
    pub remaining: i128,
}

/// Una renta. Topics: `["rented", publisher, reader]`. Da acceso hasta `until` (segundos Unix);
/// no es un `paid`. `full_price` es el precio de compra pactado para `buy_out`.
#[contractevent(topics = ["rented"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rented {
    #[topic]
    pub publisher: Address,
    #[topic]
    pub reader: Address,
    pub paper_id: String,
    pub amount: i128,
    pub fee: i128,
    pub full_price: i128,
    pub until: u64,
}

/// Una renta convertida en compra. Topics: `["bought_out", publisher, reader]`. Acompaña al
/// `paid` de la compra: `rented` es lo que ya se había pagado de renta, `amount` la diferencia
/// que se pagó ahora y `total`, el precio de compra completo.
#[contractevent(topics = ["bought_out"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoughtOut {
    #[topic]
    pub publisher: Address,
    #[topic]
    pub reader: Address,
    pub paper_id: String,
    pub rented: i128,
    pub amount: i128,
    pub total: i128,
}

#[contract]
pub struct Splitter;

#[contractimpl]
impl Splitter {
    /// `pass_day_price` y `open_day_price` son los precios por día de un pase y de un artículo
    /// abierto: los hace cumplir el contrato, así que quedan fijos como la comisión.
    pub fn __constructor(
        env: Env,
        token: Address,
        treasury: Address,
        fee_bps: u32,
        pass_day_price: i128,
        open_day_price: i128,
    ) {
        if fee_bps > MAX_FEE_BPS {
            panic_with_error!(&env, Error::FeeTooHigh);
        }
        if pass_day_price <= 0 || open_day_price <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        let config = Config {
            token,
            treasury,
            fee_bps,
            pass_day_price,
            open_day_price,
        };
        env.storage().instance().set(&DataKey::Config, &config);
    }

    /// Cobra un artículo para quien paga: `amount` sale de `payer`; la editorial recibe
    /// `amount - fee` y la tesorería `fee`. Requiere la firma de `payer` sobre esta llamada
    /// (con sus argumentos). Devuelve el reparto aplicado.
    pub fn pay(
        env: Env,
        payer: Address,
        paper_id: String,
        publisher: Address,
        amount: i128,
    ) -> Split {
        payer.require_auth();
        charge(&env, &payer, &payer, paper_id, publisher, amount)
    }

    /// Cobra varios artículos con una sola firma de `payer`. Cada artículo se reparte igual que
    /// en `pay` y deja su propio evento `paid`; las transferencias se agrupan por editorial.
    /// Si algo falla (un artículo inválido, repetido o sin saldo), no se cobra ninguno.
    /// Devuelve la suma del reparto.
    pub fn pay_batch(env: Env, payer: Address, items: Vec<Item>) -> Split {
        payer.require_auth();
        charge_batch(&env, &payer, &payer, items)
    }

    /// Cobra un artículo a `payer` y deja el acceso a nombre de `reader` (un profesor que paga
    /// la lectura de un estudiante, por ejemplo). Mismo reparto que `pay`. Solo firma `payer`:
    /// recibir un regalo no requiere la firma de `reader`.
    pub fn gift(
        env: Env,
        payer: Address,
        reader: Address,
        paper_id: String,
        publisher: Address,
        amount: i128,
    ) -> Split {
        payer.require_auth();
        charge(&env, &payer, &reader, paper_id, publisher, amount)
    }

    /// Cobra varios artículos a `payer` y deja el acceso de todos a nombre de `reader` (la lista
    /// de lectura de un curso, por ejemplo). Mismas reglas que `pay_batch`: hasta
    /// `MAX_BATCH_ITEMS`, sin repetidos, todo o nada. Solo firma `payer`.
    pub fn gift_batch(env: Env, payer: Address, reader: Address, items: Vec<Item>) -> Split {
        payer.require_auth();
        charge_batch(&env, &payer, &reader, items)
    }

    /// Cobra un artículo a `payer` y reparte la parte de la editorial entre coautores, según la
    /// parte de cada uno (`bps`; entre todos suman 10 000). La tesorería cobra su comisión como en
    /// `pay`. Cada parte se redondea hacia abajo y el residuo va al primer autor: no se pierde
    /// ningún stroop. Emite un `paid` a nombre del primer autor, así que da acceso igual que `pay`.
    /// Devuelve el reparto total.
    pub fn pay_authors(
        env: Env,
        payer: Address,
        paper_id: String,
        authors: Vec<Share>,
        amount: i128,
    ) -> Split {
        payer.require_auth();
        validate_paper_id(&env, &paper_id);
        validate_shares(&env, &authors);
        let config = read_config(&env);
        let split = compute_split(&env, amount, config.fee_bps);

        let parts = share_parts(&env, split.publisher, &authors);

        let usdc = token::Client::new(&env, &config.token);
        for (share, part) in authors.iter().zip(parts.iter()) {
            if part > 0 {
                usdc.transfer(&payer, &share.account, &part);
            }
        }
        if split.fee > 0 {
            usdc.transfer(&payer, &config.treasury, &split.fee);
        }
        extend_instance_ttl(&env);

        Paid {
            publisher: authors.get_unchecked(0).account,
            reader: payer.clone(),
            paper_id,
            payer,
            amount,
            fee: split.fee,
        }
        .publish(&env);

        split
    }

    /// Cobra un artículo como `pay` y premia a quien lo recomendó: la editorial recibe lo mismo
    /// (98%) y la comisión se parte entre `referrer` (`REFERRAL_SHARE_BPS`, la mitad, redondeada
    /// hacia abajo) y la tesorería (el resto). Emite `paid` (con la comisión total, da acceso a
    /// `payer`) y `referred`. Solo firma `payer`. Devuelve el reparto con la parte del referido.
    pub fn pay_ref(
        env: Env,
        payer: Address,
        paper_id: String,
        publisher: Address,
        amount: i128,
        referrer: Address,
    ) -> RefSplit {
        payer.require_auth();
        if referrer == payer {
            panic_with_error!(&env, Error::SelfReferral);
        }
        validate_paper_id(&env, &paper_id);
        let config = read_config(&env);
        let split = compute_split(&env, amount, config.fee_bps);
        // `fee` es un cociente entre 10 000: multiplicado por 5 000 no se desborda.
        let reward = split.fee * REFERRAL_SHARE_BPS / BPS_DENOMINATOR;
        let result = RefSplit {
            publisher: split.publisher,
            fee: split.fee - reward,
            referral: reward,
        };

        let usdc = token::Client::new(&env, &config.token);
        if result.publisher > 0 {
            usdc.transfer(&payer, &publisher, &result.publisher);
        }
        if result.referral > 0 {
            usdc.transfer(&payer, &referrer, &result.referral);
        }
        if result.fee > 0 {
            usdc.transfer(&payer, &config.treasury, &result.fee);
        }
        extend_instance_ttl(&env);

        Paid {
            publisher: publisher.clone(),
            reader: payer.clone(),
            paper_id: paper_id.clone(),
            payer: payer.clone(),
            amount,
            fee: split.fee,
        }
        .publish(&env);
        Referred {
            referrer,
            payer,
            paper_id,
            publisher,
            amount,
            reward,
        }
        .publish(&env);

        result
    }

    /// Propina de `payer` a la editorial por un artículo, con el mismo reparto que una lectura.
    /// Emite `tipped`, no `paid`: una propina no da acceso al artículo. Devuelve el reparto.
    pub fn tip(
        env: Env,
        payer: Address,
        paper_id: String,
        publisher: Address,
        amount: i128,
    ) -> Split {
        payer.require_auth();
        validate_paper_id(&env, &paper_id);
        let config = read_config(&env);
        let split = compute_split(&env, amount, config.fee_bps);
        transfer_split(&env, &config, &payer, &publisher, &split);
        extend_instance_ttl(&env);

        Tipped {
            publisher,
            payer,
            paper_id,
            amount,
            fee: split.fee,
        }
        .publish(&env);

        split
    }

    /// La editorial le devuelve `amount` a `reader` por un artículo. Solo firma la editorial.
    /// El contrato no guarda pagos, así que no comprueba el pago original: eso lo decide quien
    /// devuelve. La comisión ya cobrada no vuelve: la tesorería no firma aquí.
    pub fn refund(env: Env, publisher: Address, reader: Address, paper_id: String, amount: i128) {
        publisher.require_auth();
        validate_paper_id(&env, &paper_id);
        if amount <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(&publisher, &reader, &amount);
        extend_instance_ttl(&env);

        Refunded {
            publisher,
            reader,
            paper_id,
            amount,
        }
        .publish(&env);
    }

    /// Compra un pase de `days` días (1 a `MAX_PASS_DAYS`) para leer todo el catálogo de
    /// `publisher`. El contrato cobra exactamente `days × pass_day_price` (`amount` tiene que ser
    /// ese monto, así quien firma lo ve) y lo reparte como una lectura, sin guardar fondos. Los
    /// días se suman al pase vigente de `reader`; si ya venció, cuentan desde ahora. Un pase no
    /// vence más allá de `MAX_ACCESS_DAYS` desde ahora. `reader` puede ser otra cuenta (un pase de
    /// regalo): solo firma `payer`. Emite `pass` y devuelve el vencimiento nuevo (segundos Unix,
    /// hora del ledger).
    pub fn buy_pass(
        env: Env,
        payer: Address,
        reader: Address,
        publisher: Address,
        days: u32,
        amount: i128,
    ) -> u64 {
        payer.require_auth();
        let config = read_config(&env);
        check_price(&env, days, config.pass_day_price, amount);
        let split = compute_split(&env, amount, config.fee_bps);
        transfer_split(&env, &config, &payer, &publisher, &split);
        let valid_until = grant_pass(&env, &reader, &publisher, days);
        extend_instance_ttl(&env);

        PassBought {
            publisher,
            reader,
            payer,
            days,
            amount,
            fee: split.fee,
            valid_until,
        }
        .publish(&env);

        valid_until
    }

    /// Hasta cuándo (segundos Unix) puede leer `reader` el catálogo de `publisher`; 0 si nunca
    /// tuvo pase. Un valor menor que la hora del ledger es un pase vencido.
    pub fn pass_until(env: Env, reader: Address, publisher: Address) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::Pass(reader, publisher))
            .unwrap_or(0)
    }

    /// Suscribe a `reader` al catálogo de `publisher`: cobra ya el primer periodo (`days` días de
    /// pase por `amount`, igual que `buy_pass`) y autoriza al contrato a cobrar `renewals`
    /// renovaciones más (1 a `MAX_RENEWALS`) del mismo precio, con `approve` del token. Esa
    /// autorización vence un día después de lo que cubre la suscripción completa, que no puede
    /// pasar de `MAX_SUBSCRIPTION_DAYS`. Los fondos se quedan en la wallet de `reader` hasta cada
    /// renovación. Una suscripción nueva reemplaza a la anterior (y su autorización). Firma
    /// `reader`, una vez. Emite `pass` y `subscribed`; devuelve el vencimiento del pase.
    pub fn subscribe(
        env: Env,
        reader: Address,
        publisher: Address,
        days: u32,
        renewals: u32,
        amount: i128,
    ) -> u64 {
        reader.require_auth();
        let config = read_config(&env);
        check_price(&env, days, config.pass_day_price, amount);
        if renewals == 0 || renewals > MAX_RENEWALS {
            panic_with_error!(&env, Error::InvalidRenewals);
        }
        // Hasta 31 días × 13 periodos: cabe en u32.
        let total_days = days * (renewals + 1);
        if total_days > MAX_SUBSCRIPTION_DAYS {
            panic_with_error!(&env, Error::TooLong);
        }
        let allowance = match amount.checked_mul(renewals as i128) {
            Some(total) => total,
            None => panic_with_error!(&env, Error::AmountOverflow),
        };

        let split = compute_split(&env, amount, config.fee_bps);
        transfer_split(&env, &config, &reader, &publisher, &split);
        let valid_until = grant_pass(&env, &reader, &publisher, days);

        let allowance_until = allowance_until(&env, total_days);
        token::Client::new(&env, &config.token).approve(
            &reader,
            &env.current_contract_address(),
            &allowance,
            &allowance_until,
        );
        let key = DataKey::Sub(reader.clone());
        env.storage().persistent().set(
            &key,
            &Subscription {
                publisher: publisher.clone(),
                days,
                amount,
                renewals_left: renewals,
                allowance_until,
            },
        );
        keep_until(
            &env,
            &key,
            env.ledger().timestamp() + (total_days + 1) as u64 * DAY_SECONDS,
        );
        extend_instance_ttl(&env);

        PassBought {
            publisher: publisher.clone(),
            reader: reader.clone(),
            payer: reader.clone(),
            days,
            amount,
            fee: split.fee,
            valid_until,
        }
        .publish(&env);
        Subscribed {
            publisher,
            reader,
            days,
            amount,
            renewals,
            allowance_until,
        }
        .publish(&env);

        valid_until
    }

    /// Cobra una renovación de la suscripción de `reader` con la autorización que dio al
    /// suscribirse (`transfer_from`), la reparte como una lectura y suma `days` días a su pase.
    /// No pide firmas: la puede enviar cualquiera (la editorial, PaperPay o quien lee), porque
    /// solo cobra el precio pactado y solo cuando al pase le falta `RENEW_WINDOW_SECONDS` o menos;
    /// así nunca se cobra más de un periodo por adelantado. Con la última renovación, la
    /// suscripción termina. Emite `pass` y `renewed`; devuelve el vencimiento nuevo del pase.
    pub fn renew_subscription(env: Env, reader: Address) -> u64 {
        let key = DataKey::Sub(reader.clone());
        let mut sub: Subscription = match env.storage().persistent().get(&key) {
            Some(sub) => sub,
            None => panic_with_error!(&env, Error::NoSubscription),
        };
        let current: u64 = env
            .storage()
            .persistent()
            .get(&DataKey::Pass(reader.clone(), sub.publisher.clone()))
            .unwrap_or(0);
        if current > env.ledger().timestamp() + RENEW_WINDOW_SECONDS {
            panic_with_error!(&env, Error::RenewTooEarly);
        }

        let config = read_config(&env);
        let split = compute_split(&env, sub.amount, config.fee_bps);
        let usdc = token::Client::new(&env, &config.token);
        let spender = env.current_contract_address();
        if split.publisher > 0 {
            usdc.transfer_from(&spender, &reader, &sub.publisher, &split.publisher);
        }
        if split.fee > 0 {
            usdc.transfer_from(&spender, &reader, &config.treasury, &split.fee);
        }
        let valid_until = grant_pass(&env, &reader, &sub.publisher, sub.days);

        sub.renewals_left -= 1;
        if sub.renewals_left == 0 {
            env.storage().persistent().remove(&key);
        } else {
            env.storage().persistent().set(&key, &sub);
        }
        extend_instance_ttl(&env);

        PassBought {
            publisher: sub.publisher.clone(),
            reader: reader.clone(),
            payer: reader.clone(),
            days: sub.days,
            amount: sub.amount,
            fee: split.fee,
            valid_until,
        }
        .publish(&env);
        Renewed {
            publisher: sub.publisher,
            reader,
            days: sub.days,
            amount: sub.amount,
            fee: split.fee,
            valid_until,
            renewals_left: sub.renewals_left,
        }
        .publish(&env);

        valid_until
    }

    /// Cancela la suscripción de `reader`: borra las renovaciones pendientes y deja en cero la
    /// autorización del token. El pase ya pagado sigue hasta su vencimiento. Firma `reader`.
    /// Emite `unsubscribed`.
    pub fn cancel_subscription(env: Env, reader: Address) {
        reader.require_auth();
        let key = DataKey::Sub(reader.clone());
        let sub: Subscription = match env.storage().persistent().get(&key) {
            Some(sub) => sub,
            None => panic_with_error!(&env, Error::NoSubscription),
        };
        env.storage().persistent().remove(&key);
        let config = read_config(&env);
        // Monto 0 con vencimiento 0: el token lo acepta y la llamada firmada no depende del
        // ledger en que se envía.
        token::Client::new(&env, &config.token).approve(
            &reader,
            &env.current_contract_address(),
            &0,
            &0,
        );
        extend_instance_ttl(&env);

        Unsubscribed {
            publisher: sub.publisher,
            reader,
            renewals_left: sub.renewals_left,
        }
        .publish(&env);
    }

    /// La suscripción vigente de `reader`, si tiene.
    pub fn subscription(env: Env, reader: Address) -> Option<Subscription> {
        env.storage().persistent().get(&DataKey::Sub(reader))
    }

    /// `sponsor` paga para que `paper_id` de `publisher` quede abierto para todos `days` días
    /// (1 a `MAX_PASS_DAYS`): un laboratorio, una fundación o la misma editorial patrocina el
    /// acceso abierto. Cobra exactamente `days × open_day_price` y lo reparte como una lectura,
    /// sin guardar fondos. Los días se suman a la apertura vigente; si ya venció, cuentan desde
    /// ahora, y no pasa de `MAX_ACCESS_DAYS` desde ahora. Solo firma `sponsor`. Emite `opened` (no
    /// `paid`: no es la lectura de una cuenta) y devuelve hasta cuándo queda abierto.
    pub fn sponsor_open(
        env: Env,
        sponsor: Address,
        paper_id: String,
        publisher: Address,
        days: u32,
        amount: i128,
    ) -> u64 {
        sponsor.require_auth();
        validate_paper_id(&env, &paper_id);
        let config = read_config(&env);
        check_price(&env, days, config.open_day_price, amount);
        let split = compute_split(&env, amount, config.fee_bps);
        transfer_split(&env, &config, &sponsor, &publisher, &split);
        let open_until = extend_access(
            &env,
            &DataKey::Open(publisher.clone(), paper_id.clone()),
            days,
        );
        extend_instance_ttl(&env);

        Opened {
            publisher,
            sponsor,
            paper_id,
            days,
            amount,
            fee: split.fee,
            open_until,
        }
        .publish(&env);

        open_until
    }

    /// Hasta cuándo (segundos Unix) está abierto para todos `paper_id` de `publisher`; 0 si nunca
    /// se abrió.
    pub fn open_until(env: Env, publisher: Address, paper_id: String) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::Open(publisher, paper_id))
            .unwrap_or(0)
    }

    /// `publisher` ofrece liberar `paper_id` para todos si los aportes llegan a `goal` antes de
    /// `deadline` (segundos Unix, hora del ledger; como mucho `MAX_CAMPAIGN_DAYS` desde ahora). Su
    /// firma es la oferta pública: nadie más puede abrir una campaña por su artículo. Los ids son
    /// secuenciales, así que nadie puede ocupar el id de la campaña de otra editorial. No mueve
    /// fondos. Emite `campaign` y devuelve el id.
    pub fn open_campaign(
        env: Env,
        publisher: Address,
        paper_id: String,
        goal: i128,
        deadline: u64,
    ) -> u32 {
        publisher.require_auth();
        validate_paper_id(&env, &paper_id);
        if goal <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        let now = env.ledger().timestamp();
        if deadline <= now || deadline - now > MAX_CAMPAIGN_DAYS as u64 * DAY_SECONDS {
            panic_with_error!(&env, Error::InvalidDeadline);
        }

        let id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextCampaign)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::NextCampaign, &(id + 1));
        let key = DataKey::Campaign(id);
        env.storage().persistent().set(
            &key,
            &Campaign {
                publisher: publisher.clone(),
                paper_id: paper_id.clone(),
                goal,
                deadline,
                raised: 0,
                released: false,
            },
        );
        keep_campaign(&env, &key, deadline);
        extend_instance_ttl(&env);

        CampaignOpened {
            publisher,
            campaign_id: id,
            paper_id,
            goal,
            deadline,
        }
        .publish(&env);

        id
    }

    /// `backer` aporta `amount` a la campaña `campaign_id`. El aporte pasa de su wallet al
    /// contrato y queda en garantía: si se llega a la meta, va a la editorial (`release_pledges`);
    /// si vence el plazo sin llegar, vuelve completo a `backer` (`refund_pledge`). Solo mientras la
    /// campaña está abierta (antes del plazo, sin la meta cumplida y sin liberar) y sin pasar de
    /// lo que falta (`OverGoal`): el monto firmado es el que se cobra, nunca se recorta. Solo firma
    /// `backer`. Emite `pledged` y devuelve lo aportado a la campaña hasta ahora.
    pub fn pledge(env: Env, backer: Address, campaign_id: u32, amount: i128) -> i128 {
        backer.require_auth();
        if amount <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        let key = DataKey::Campaign(campaign_id);
        let mut campaign = read_campaign(&env, campaign_id);
        if campaign.released
            || env.ledger().timestamp() >= campaign.deadline
            || campaign.raised >= campaign.goal
        {
            panic_with_error!(&env, Error::CampaignClosed);
        }
        if amount > campaign.goal - campaign.raised {
            panic_with_error!(&env, Error::OverGoal);
        }

        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &backer,
            env.current_contract_address(),
            &amount,
        );
        // Ni el total ni el aporte de una cuenta pasan de la meta: no se desbordan.
        campaign.raised += amount;
        let pledge_key = DataKey::Pledge(campaign_id, backer.clone());
        let pledged: i128 = env.storage().persistent().get(&pledge_key).unwrap_or(0);
        env.storage()
            .persistent()
            .set(&pledge_key, &(pledged + amount));
        env.storage().persistent().set(&key, &campaign);
        keep_campaign(&env, &key, campaign.deadline);
        keep_campaign(&env, &pledge_key, campaign.deadline);
        extend_instance_ttl(&env);

        Pledged {
            backer,
            campaign_id,
            amount,
            raised: campaign.raised,
        }
        .publish(&env);

        campaign.raised
    }

    /// Con la meta cumplida, paga lo juntado: el 98% a la editorial y la comisión a la tesorería,
    /// y marca la campaña como liberada (el artículo queda abierto para todos). No pide firmas:
    /// la puede enviar cualquiera, porque el dinero solo puede ir a la editorial de la campaña y a
    /// la tesorería. Se puede liberar en cuanto se llega a la meta, antes o después del plazo, y
    /// una sola vez. Si la editorial no puede recibir (sin trustline), su parte queda pendiente a
    /// su nombre (`credited`). Emite `released` y devuelve el reparto.
    pub fn release_pledges(env: Env, campaign_id: u32) -> Split {
        let key = DataKey::Campaign(campaign_id);
        let mut campaign = read_campaign(&env, campaign_id);
        if campaign.released {
            panic_with_error!(&env, Error::CampaignClosed);
        }
        if campaign.raised < campaign.goal {
            panic_with_error!(&env, Error::GoalNotReached);
        }
        campaign.released = true;
        env.storage().persistent().set(&key, &campaign);

        let config = read_config(&env);
        let split = compute_split(&env, campaign.raised, config.fee_bps);
        payout(&env, &config, &campaign.publisher, split.publisher);
        payout(&env, &config, &config.treasury, split.fee);
        extend_instance_ttl(&env);

        Released {
            publisher: campaign.publisher,
            campaign_id,
            paper_id: campaign.paper_id,
            raised: campaign.raised,
            fee: split.fee,
        }
        .publish(&env);

        split
    }

    /// Devuelve el aporte de `backer` a una campaña que falló: venció el plazo sin llegar a la
    /// meta. No pide firmas: la puede enviar cualquiera (el sitio, la editorial o quien aportó),
    /// porque el dinero solo puede volver a `backer`. Si `backer` ya no puede recibir, el aporte
    /// queda pendiente a su nombre (`credited`) para `claim`. Emite `unpledged` y devuelve el monto.
    pub fn refund_pledge(env: Env, campaign_id: u32, backer: Address) -> i128 {
        let key = DataKey::Campaign(campaign_id);
        let mut campaign = read_campaign(&env, campaign_id);
        if campaign.released
            || campaign.raised >= campaign.goal
            || env.ledger().timestamp() < campaign.deadline
        {
            panic_with_error!(&env, Error::CampaignNotFailed);
        }
        let pledge_key = DataKey::Pledge(campaign_id, backer.clone());
        let pledged: i128 = env.storage().persistent().get(&pledge_key).unwrap_or(0);
        if pledged <= 0 {
            panic_with_error!(&env, Error::NothingToRefund);
        }
        env.storage().persistent().remove(&pledge_key);
        campaign.raised -= pledged;
        env.storage().persistent().set(&key, &campaign);

        let config = read_config(&env);
        payout(&env, &config, &backer, pledged);
        extend_instance_ttl(&env);

        Unpledged {
            backer,
            campaign_id,
            amount: pledged,
        }
        .publish(&env);

        pledged
    }

    /// `account` cobra todo lo que tiene pendiente en el contrato. Con la misma firma le abre la
    /// trustline de USDC si no la tiene (`trust` del SAC, CAP-73): así cobra una editorial nueva,
    /// sin preparar su cuenta antes. La cuenta necesita XLM libre para la reserva de la trustline;
    /// si no puede recibir, la transacción falla y lo pendiente sigue intacto. Firma `account`.
    /// Emite `claimed` y devuelve lo cobrado.
    pub fn claim(env: Env, account: Address) -> i128 {
        account.require_auth();
        let key = DataKey::Owed(account.clone());
        let owed: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        if owed <= 0 {
            panic_with_error!(&env, Error::NothingToClaim);
        }
        env.storage().persistent().remove(&key);

        let config = read_config(&env);
        token::StellarAssetClient::new(&env, &config.token).trust(&account);
        token::Client::new(&env, &config.token).transfer(
            &env.current_contract_address(),
            &account,
            &owed,
        );
        extend_instance_ttl(&env);

        Claimed {
            account,
            amount: owed,
        }
        .publish(&env);

        owed
    }

    /// `sponsor` deposita `amount` en su fondo para pagar lecturas de `publisher`. El dinero pasa
    /// de su wallet al contrato y solo tiene dos salidas: una lectura de esa editorial
    /// (`pay_sponsored`) o de vuelta a `sponsor` (`withdraw_pool`). `operator` es la única cuenta
    /// que puede gastarlo, hasta `max_per_read` por lectura. Un depósito nuevo se suma al saldo y
    /// reemplaza al operador y al tope. Solo firma `sponsor`. Emite `pool_funded` y devuelve el
    /// saldo del fondo.
    pub fn fund_pool(
        env: Env,
        sponsor: Address,
        publisher: Address,
        operator: Address,
        max_per_read: i128,
        amount: i128,
    ) -> i128 {
        sponsor.require_auth();
        if amount <= 0 || max_per_read <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        let key = DataKey::Pool(sponsor.clone(), publisher.clone());
        let before: i128 = match env.storage().persistent().get::<DataKey, Pool>(&key) {
            Some(pool) => pool.balance,
            None => 0,
        };
        let balance = add(&env, before, amount);

        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &sponsor,
            env.current_contract_address(),
            &amount,
        );
        env.storage().persistent().set(
            &key,
            &Pool {
                operator: operator.clone(),
                max_per_read,
                balance,
            },
        );
        keep_max(&env, &key);
        extend_instance_ttl(&env);

        PoolFunded {
            sponsor,
            publisher,
            operator,
            max_per_read,
            amount,
            balance,
        }
        .publish(&env);

        balance
    }

    /// El operador del fondo de `sponsor` paga una lectura de `paper_id` para `reader`: `amount`
    /// sale del fondo y se reparte 98/2 entre `publisher` y la tesorería, directo desde el
    /// contrato. Ni `sponsor` ni `reader` firman: solo `operator`, que tiene que ser el del fondo.
    /// El contrato hace cumplir las reglas del fondo: solo paga a su editorial, no más de
    /// `max_per_read` por lectura, no más de lo que queda y una sola vez por artículo y lector
    /// cada 24 h. Si la editorial no puede recibir, falla y el fondo queda intacto. Emite `paid`
    /// (con `payer = sponsor`) y `sponsored`, y devuelve el reparto.
    pub fn pay_sponsored(
        env: Env,
        operator: Address,
        sponsor: Address,
        reader: Address,
        paper_id: String,
        publisher: Address,
        amount: i128,
    ) -> Split {
        operator.require_auth();
        validate_paper_id(&env, &paper_id);
        let key = DataKey::Pool(sponsor.clone(), publisher.clone());
        let mut pool = read_pool(&env, &key);
        if pool.operator != operator {
            panic_with_error!(&env, Error::NotOperator);
        }
        let config = read_config(&env);
        let split = compute_split(&env, amount, config.fee_bps);
        if amount > pool.max_per_read {
            panic_with_error!(&env, Error::AboveMaxPerRead);
        }
        if amount > pool.balance {
            panic_with_error!(&env, Error::PoolTooLow);
        }
        let marker = DataKey::Sponsored(sponsor.clone(), reader.clone(), paper_id.clone());
        if env.storage().temporary().has(&marker) {
            panic_with_error!(&env, Error::AlreadySponsored);
        }

        pool.balance -= amount;
        env.storage().persistent().set(&key, &pool);
        keep_max(&env, &key);
        env.storage().temporary().set(&marker, &true);
        env.storage()
            .temporary()
            .extend_ttl(&marker, SPONSORED_TTL_LEDGERS, SPONSORED_TTL_LEDGERS);
        transfer_split(
            &env,
            &config,
            &env.current_contract_address(),
            &publisher,
            &split,
        );
        extend_instance_ttl(&env);

        Paid {
            publisher: publisher.clone(),
            reader: reader.clone(),
            paper_id: paper_id.clone(),
            payer: sponsor.clone(),
            amount,
            fee: split.fee,
        }
        .publish(&env);
        SponsoredRead {
            sponsor,
            reader,
            paper_id,
            publisher,
            amount,
            balance: pool.balance,
        }
        .publish(&env);

        split
    }

    /// `sponsor` retira `amount` de su fondo para `publisher`: vuelve del contrato a su wallet.
    /// Puede retirar una parte o todo, cuando quiera; el operador no puede retirar. Solo firma
    /// `sponsor`. Emite `pool_withdrawn` y devuelve lo que queda en el fondo.
    pub fn withdraw_pool(env: Env, sponsor: Address, publisher: Address, amount: i128) -> i128 {
        sponsor.require_auth();
        if amount <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        let key = DataKey::Pool(sponsor.clone(), publisher.clone());
        let mut pool = read_pool(&env, &key);
        if amount > pool.balance {
            panic_with_error!(&env, Error::PoolTooLow);
        }
        pool.balance -= amount;
        env.storage().persistent().set(&key, &pool);
        keep_max(&env, &key);

        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &env.current_contract_address(),
            &sponsor,
            &amount,
        );
        extend_instance_ttl(&env);

        PoolWithdrawn {
            sponsor,
            publisher,
            amount,
            balance: pool.balance,
        }
        .publish(&env);

        pool.balance
    }

    /// El fondo de patrocinio de `sponsor` para `publisher`, si existe.
    pub fn pool(env: Env, sponsor: Address, publisher: Address) -> Option<Pool> {
        env.storage()
            .persistent()
            .get(&DataKey::Pool(sponsor, publisher))
    }

    /// `funder` publica una recompensa de `amount` por revisar o reproducir `paper_id`. El dinero
    /// pasa de su wallet al contrato y queda en garantía hasta `deadline` (segundos Unix, a no más
    /// de `MAX_BOUNTY_DAYS`): antes del plazo solo puede salir hacia quienes ganen
    /// (`award_bounty`); después, también puede volver a `funder` (`cancel_bounty`). Solo firma
    /// `funder`. Emite `bounty_posted` y devuelve el id de la recompensa.
    pub fn post_bounty(
        env: Env,
        funder: Address,
        paper_id: String,
        amount: i128,
        deadline: u64,
    ) -> u32 {
        funder.require_auth();
        validate_paper_id(&env, &paper_id);
        if amount <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        let now = env.ledger().timestamp();
        if deadline <= now || deadline - now > MAX_BOUNTY_DAYS as u64 * DAY_SECONDS {
            panic_with_error!(&env, Error::InvalidDeadline);
        }

        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &funder,
            env.current_contract_address(),
            &amount,
        );
        let id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextBounty)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::NextBounty, &(id + 1));
        let key = DataKey::Bounty(id);
        env.storage().persistent().set(
            &key,
            &Bounty {
                funder: funder.clone(),
                paper_id: paper_id.clone(),
                amount,
                deadline,
                open: true,
            },
        );
        keep_campaign(&env, &key, deadline);
        extend_instance_ttl(&env);

        BountyPosted {
            funder,
            bounty_id: id,
            paper_id,
            amount,
            deadline,
        }
        .publish(&env);

        id
    }

    /// `funder` reparte su recompensa `bounty_id` entre `winners` (de 1 a `MAX_AUTHORS` cuentas,
    /// con partes en puntos base que suman 100%): la tesorería recibe la comisión y los ganadores
    /// el resto, cada uno su parte. Quién gana lo decide quien publicó la recompensa; el contrato
    /// garantiza que el dinero existe y que sale completo, una sola vez. `funder` no puede ser
    /// ganador. Se puede repartir antes o después del plazo, mientras no se haya cancelado. Si un
    /// ganador no puede recibir (sin trustline), su parte queda pendiente a su nombre para
    /// `claim`. Solo firma `funder`. Emite un `rewarded` por ganador y `bounty_awarded`; devuelve
    /// el reparto (lo que va a los ganadores y la comisión).
    pub fn award_bounty(env: Env, funder: Address, bounty_id: u32, winners: Vec<Share>) -> Split {
        funder.require_auth();
        let key = DataKey::Bounty(bounty_id);
        let mut bounty = read_bounty(&env, &key, &funder);
        validate_shares(&env, &winners);
        for share in winners.iter() {
            if share.account == funder {
                panic_with_error!(&env, Error::InvalidShares);
            }
        }
        bounty.open = false;
        env.storage().persistent().set(&key, &bounty);

        let config = read_config(&env);
        let split = compute_split(&env, bounty.amount, config.fee_bps);
        let parts = share_parts(&env, split.publisher, &winners);
        for (share, part) in winners.iter().zip(parts.iter()) {
            payout(&env, &config, &share.account, part);
            Rewarded {
                winner: share.account,
                bounty_id,
                amount: part,
            }
            .publish(&env);
        }
        payout(&env, &config, &config.treasury, split.fee);
        extend_instance_ttl(&env);

        BountyAwarded {
            funder,
            bounty_id,
            paper_id: bounty.paper_id,
            amount: bounty.amount,
            fee: split.fee,
        }
        .publish(&env);

        split
    }

    /// `funder` cancela su recompensa `bounty_id` y recupera todo, sin comisión. Solo después del
    /// plazo (`BountyActive` antes) y si no se repartió. Solo firma `funder`. Emite
    /// `bounty_cancelled` y devuelve lo recuperado.
    pub fn cancel_bounty(env: Env, funder: Address, bounty_id: u32) -> i128 {
        funder.require_auth();
        let key = DataKey::Bounty(bounty_id);
        let mut bounty = read_bounty(&env, &key, &funder);
        if env.ledger().timestamp() < bounty.deadline {
            panic_with_error!(&env, Error::BountyActive);
        }
        bounty.open = false;
        env.storage().persistent().set(&key, &bounty);

        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &env.current_contract_address(),
            &funder,
            &bounty.amount,
        );
        extend_instance_ttl(&env);

        BountyCancelled {
            funder,
            bounty_id,
            amount: bounty.amount,
        }
        .publish(&env);

        bounty.amount
    }

    /// La recompensa `bounty_id`, si existe.
    pub fn bounty(env: Env, bounty_id: u32) -> Option<Bounty> {
        env.storage().persistent().get(&DataKey::Bounty(bounty_id))
    }

    /// `buyer` compra un vale por cada llave de `keys` (de 1 a `MAX_BATCH_ITEMS`): cada vale paga
    /// una lectura de `value` del catálogo de `publisher`, hasta `expires_at` (segundos Unix, a no
    /// más de `MAX_VOUCHER_DAYS`). El total pasa de su wallet al contrato. Cada llave es la parte
    /// pública de un par ed25519 que `buyer` genera y reparte (por ejemplo, como enlace): quien
    /// tenga la parte privada puede canjear el vale. Una llave no se puede usar dos veces. Solo
    /// firma `buyer`. Emite `vouchers_bought` y devuelve el total pagado.
    pub fn buy_vouchers(
        env: Env,
        buyer: Address,
        publisher: Address,
        keys: Vec<BytesN<32>>,
        value: i128,
        expires_at: u64,
    ) -> i128 {
        buyer.require_auth();
        check_keys(&env, &keys);
        if value <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        let now = env.ledger().timestamp();
        if expires_at <= now || expires_at - now > MAX_VOUCHER_DAYS as u64 * DAY_SECONDS {
            panic_with_error!(&env, Error::InvalidDeadline);
        }
        let total = match value.checked_mul(keys.len() as i128) {
            Some(total) => total,
            None => panic_with_error!(&env, Error::AmountOverflow),
        };
        for key in keys.iter() {
            let entry = DataKey::Voucher(key);
            // También detecta una llave repetida en la misma lista: la primera ya quedó guardada.
            if env.storage().persistent().has(&entry) {
                panic_with_error!(&env, Error::VoucherExists);
            }
            env.storage().persistent().set(
                &entry,
                &Voucher {
                    buyer: buyer.clone(),
                    publisher: publisher.clone(),
                    value,
                    expires_at,
                    open: true,
                },
            );
            keep_campaign(&env, &entry, expires_at);
        }

        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &buyer,
            env.current_contract_address(),
            &total,
        );
        extend_instance_ttl(&env);

        VouchersBought {
            buyer,
            publisher,
            keys,
            value,
            expires_at,
            total,
        }
        .publish(&env);

        total
    }

    /// Canjea el vale de `key`: paga una lectura de `paper_id` para `reader`, 98/2 directo desde
    /// el contrato a la editorial del vale y a la tesorería. No pide la firma de ninguna cuenta:
    /// la autorización es `signature`, la firma ed25519 de la llave del vale sobre
    /// `(este contrato, reader, paper_id)` en XDR. Esa firma solo sirve para ese lector y ese
    /// artículo, así que cualquiera la puede enviar (el sitio, por ejemplo, para que quien lee no
    /// necesite cuenta). Un vale se canjea una vez y antes de vencer. Si la editorial no puede
    /// recibir, falla y el vale sigue vigente. Emite `paid` (con `payer` = quien compró el vale) y
    /// `voucher_redeemed`, y devuelve el reparto.
    pub fn redeem_voucher(
        env: Env,
        key: BytesN<32>,
        signature: BytesN<64>,
        reader: Address,
        paper_id: String,
    ) -> Split {
        validate_paper_id(&env, &paper_id);
        let entry = DataKey::Voucher(key.clone());
        let mut voucher = read_voucher(&env, &entry);
        if env.ledger().timestamp() >= voucher.expires_at {
            panic_with_error!(&env, Error::VoucherExpired);
        }
        let message = (
            env.current_contract_address(),
            reader.clone(),
            paper_id.clone(),
        )
            .to_xdr(&env);
        // Si la firma no es de esa llave sobre ese mensaje, la transacción falla aquí.
        env.crypto().ed25519_verify(&key, &message, &signature);
        voucher.open = false;
        env.storage().persistent().set(&entry, &voucher);

        let config = read_config(&env);
        let split = compute_split(&env, voucher.value, config.fee_bps);
        transfer_split(
            &env,
            &config,
            &env.current_contract_address(),
            &voucher.publisher,
            &split,
        );
        extend_instance_ttl(&env);

        Paid {
            publisher: voucher.publisher,
            reader: reader.clone(),
            paper_id: paper_id.clone(),
            payer: voucher.buyer.clone(),
            amount: voucher.value,
            fee: split.fee,
        }
        .publish(&env);
        VoucherRedeemed {
            buyer: voucher.buyer,
            reader,
            key,
            paper_id,
            value: voucher.value,
        }
        .publish(&env);

        split
    }

    /// `buyer` recupera los vales de `keys` que vencieron sin canjearse: su valor vuelve del
    /// contrato a su wallet, sin comisión. Todos tienen que ser suyos, estar sin usar y vencidos
    /// (`VoucherActive` antes); si uno no cumple, no se devuelve ninguno. Solo firma `buyer`.
    /// Emite `vouchers_reclaimed` y devuelve el total recuperado.
    pub fn reclaim_vouchers(env: Env, buyer: Address, keys: Vec<BytesN<32>>) -> i128 {
        buyer.require_auth();
        check_keys(&env, &keys);
        let now = env.ledger().timestamp();
        let mut total: i128 = 0;
        for key in keys.iter() {
            let entry = DataKey::Voucher(key);
            let mut voucher = read_voucher(&env, &entry);
            if voucher.buyer != buyer {
                panic_with_error!(&env, Error::NoVoucher);
            }
            if now < voucher.expires_at {
                panic_with_error!(&env, Error::VoucherActive);
            }
            // Una llave repetida en la lista encuentra el vale ya cerrado: `VoucherSpent`.
            voucher.open = false;
            env.storage().persistent().set(&entry, &voucher);
            total = add(&env, total, voucher.value);
        }

        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &env.current_contract_address(),
            &buyer,
            &total,
        );
        extend_instance_ttl(&env);

        VouchersReclaimed { buyer, keys, total }.publish(&env);

        total
    }

    /// El vale de `key`, si existe.
    pub fn voucher(env: Env, key: BytesN<32>) -> Option<Voucher> {
        env.storage().persistent().get(&DataKey::Voucher(key))
    }

    /// `reader` abre una cuenta para leer ahora y pagar después: deja `deposit` en el contrato como
    /// garantía y nombra a `operator` (el sitio), la única cuenta que puede anotarle lecturas. La
    /// deuda nunca pasa de la garantía. Desde la primera lectura sin pagar tiene `period` segundos
    /// (hasta `MAX_PASS_DAYS` días) para pagar con `settle_tab`; si no, cualquiera cobra de la
    /// garantía con `collect_tab`. Una cuenta por lector. Solo firma `reader`. Emite `tab_opened`.
    pub fn open_tab(env: Env, reader: Address, operator: Address, deposit: i128, period: u64) {
        reader.require_auth();
        if deposit <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        if period == 0 || period > MAX_PASS_DAYS as u64 * DAY_SECONDS {
            panic_with_error!(&env, Error::InvalidDays);
        }
        let key = DataKey::Tab(reader.clone());
        if env.storage().persistent().has(&key) {
            panic_with_error!(&env, Error::TabExists);
        }

        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &reader,
            env.current_contract_address(),
            &deposit,
        );
        env.storage().persistent().set(
            &key,
            &Tab {
                operator: operator.clone(),
                deposit,
                period,
                owed: 0,
                due: 0,
                debts: Map::new(&env),
            },
        );
        keep_max(&env, &key);
        extend_instance_ttl(&env);

        TabOpened {
            reader,
            operator,
            deposit,
            period,
        }
        .publish(&env);
    }

    /// De soporte (no mueve fondos): el operador anota en la cuenta de `reader` una lectura de
    /// `paper_id` por `amount`, como deuda con `publisher`. Falla si la deuda pasaría de la
    /// garantía o si ya hay `MAX_TAB_PUBLISHERS` editoriales con deuda (`TabFull`). La primera
    /// lectura sin pagar arranca el plazo de pago. Solo firma `operator`. Emite `charged` y
    /// devuelve la deuda total.
    pub fn charge_tab(
        env: Env,
        operator: Address,
        reader: Address,
        publisher: Address,
        paper_id: String,
        amount: i128,
    ) -> i128 {
        operator.require_auth();
        validate_paper_id(&env, &paper_id);
        if amount <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        let key = DataKey::Tab(reader.clone());
        let mut tab = read_tab(&env, &key);
        if tab.operator != operator {
            panic_with_error!(&env, Error::NotOperator);
        }
        let owed = add(&env, tab.owed, amount);
        let debt = tab.debts.get(publisher.clone()).unwrap_or(0);
        if owed > tab.deposit || (debt == 0 && tab.debts.len() >= MAX_TAB_PUBLISHERS) {
            panic_with_error!(&env, Error::TabFull);
        }
        if tab.owed == 0 {
            tab.due = env.ledger().timestamp() + tab.period;
        }
        tab.owed = owed;
        // La deuda con una editorial es parte de `owed`, que ya se sumó sin desbordarse.
        tab.debts.set(publisher.clone(), debt + amount);
        env.storage().persistent().set(&key, &tab);
        keep_max(&env, &key);
        extend_instance_ttl(&env);

        Charged {
            reader,
            publisher,
            paper_id,
            amount,
            owed,
            due: tab.due,
        }
        .publish(&env);

        owed
    }

    /// `reader` paga toda la deuda de su cuenta abierta con una firma: el total sale de su wallet
    /// y cada editorial recibe el 98% de lo suyo; la tesorería, el 2%. La garantía no se toca y
    /// la cuenta sigue abierta, sin deuda. Si una editorial no puede recibir, su parte queda
    /// pendiente a su nombre para `claim`. Solo firma `reader`. Emite `tab_settled` y devuelve el
    /// reparto.
    pub fn settle_tab(env: Env, reader: Address) -> Split {
        reader.require_auth();
        let key = DataKey::Tab(reader.clone());
        let mut tab = read_tab(&env, &key);
        if tab.owed <= 0 {
            panic_with_error!(&env, Error::NothingOwed);
        }
        let total = tab.owed;
        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &reader,
            env.current_contract_address(),
            &total,
        );
        let (split, publishers) = pay_debts(&env, &config, &mut tab);
        env.storage().persistent().set(&key, &tab);
        keep_max(&env, &key);
        extend_instance_ttl(&env);

        TabSettled {
            reader,
            total,
            fee: split.fee,
            publishers,
        }
        .publish(&env);

        split
    }

    /// Cobra de la garantía la deuda vencida de la cuenta abierta de `reader`: cada editorial
    /// recibe el 98% de lo suyo y la tesorería el 2%, y la garantía baja en ese total. Solo
    /// después del plazo de pago (`TabNotDue` antes). No pide firmas: la puede enviar cualquiera
    /// (el sitio o una editorial), porque el dinero solo va a quienes se les debe. Emite
    /// `tab_collected` y devuelve el reparto.
    pub fn collect_tab(env: Env, reader: Address) -> Split {
        let key = DataKey::Tab(reader.clone());
        let mut tab = read_tab(&env, &key);
        if tab.owed <= 0 {
            panic_with_error!(&env, Error::NothingOwed);
        }
        if env.ledger().timestamp() < tab.due {
            panic_with_error!(&env, Error::TabNotDue);
        }
        let total = tab.owed;
        // La deuda nunca pasa de la garantía (`charge_tab`).
        tab.deposit -= total;
        let config = read_config(&env);
        let (split, publishers) = pay_debts(&env, &config, &mut tab);
        env.storage().persistent().set(&key, &tab);
        keep_max(&env, &key);
        extend_instance_ttl(&env);

        TabCollected {
            reader,
            total,
            fee: split.fee,
            publishers,
            deposit: tab.deposit,
        }
        .publish(&env);

        split
    }

    /// `reader` cierra su cuenta abierta y recupera la garantía que queda. Solo sin deuda
    /// (`TabOwes` si debe algo). Solo firma `reader`. Emite `tab_closed` y devuelve lo devuelto.
    pub fn close_tab(env: Env, reader: Address) -> i128 {
        reader.require_auth();
        let key = DataKey::Tab(reader.clone());
        let tab = read_tab(&env, &key);
        if tab.owed > 0 {
            panic_with_error!(&env, Error::TabOwes);
        }
        env.storage().persistent().remove(&key);

        if tab.deposit > 0 {
            let config = read_config(&env);
            token::Client::new(&env, &config.token).transfer(
                &env.current_contract_address(),
                &reader,
                &tab.deposit,
            );
        }
        extend_instance_ttl(&env);

        TabClosed {
            reader,
            amount: tab.deposit,
        }
        .publish(&env);

        tab.deposit
    }

    /// La cuenta abierta de `reader`, si existe.
    pub fn tab(env: Env, reader: Address) -> Option<Tab> {
        env.storage().persistent().get(&DataKey::Tab(reader))
    }

    /// `reader` compra un Pase PaperPay: paga `amount` por leer todo el catálogo durante `period`
    /// segundos (hasta `MAX_PASS_DAYS` días). El dinero pasa de su wallet al contrato y queda en
    /// garantía hasta que vence: entonces se reparte entre las editoriales según lo que leyó
    /// (`settle_pass`). `operator` (el sitio) es la única cuenta que puede anotarle lecturas, y es
    /// quien decide si `amount` alcanza, igual que decide el precio de cada artículo. Un pase a
    /// la vez. Solo firma `reader`. Emite `global_pass` y devuelve cuándo vence.
    pub fn buy_global_pass(
        env: Env,
        reader: Address,
        operator: Address,
        amount: i128,
        period: u64,
    ) -> u64 {
        reader.require_auth();
        if amount <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        if period == 0 || period > MAX_PASS_DAYS as u64 * DAY_SECONDS {
            panic_with_error!(&env, Error::InvalidDays);
        }
        let key = DataKey::GlobalPass(reader.clone());
        if env.storage().persistent().has(&key) {
            panic_with_error!(&env, Error::PassExists);
        }

        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &reader,
            env.current_contract_address(),
            &amount,
        );
        let until = env.ledger().timestamp() + period;
        env.storage().persistent().set(
            &key,
            &GlobalPass {
                operator: operator.clone(),
                amount,
                until,
                reads: 0,
                by_publisher: Map::new(&env),
            },
        );
        keep_campaign(&env, &key, until);
        extend_instance_ttl(&env);

        GlobalPassBought {
            reader,
            operator,
            amount,
            until,
        }
        .publish(&env);

        until
    }

    /// De soporte (no mueve fondos): el operador anota que `reader` leyó `paper_id`, de
    /// `publisher`, con su Pase PaperPay. Solo mientras el pase está vigente (`PassExpired`
    /// después) y hasta `MAX_TAB_PUBLISHERS` editoriales distintas (`PassFull`). Solo firma
    /// `operator`. Emite `pass_read` y devuelve el total de lecturas del pase.
    pub fn use_pass(
        env: Env,
        operator: Address,
        reader: Address,
        publisher: Address,
        paper_id: String,
    ) -> u32 {
        operator.require_auth();
        validate_paper_id(&env, &paper_id);
        let key = DataKey::GlobalPass(reader.clone());
        let mut pass = read_global_pass(&env, &key);
        if pass.operator != operator {
            panic_with_error!(&env, Error::NotOperator);
        }
        if env.ledger().timestamp() >= pass.until {
            panic_with_error!(&env, Error::PassExpired);
        }
        let count = pass.by_publisher.get(publisher.clone()).unwrap_or(0);
        if count == 0 && pass.by_publisher.len() >= MAX_TAB_PUBLISHERS {
            panic_with_error!(&env, Error::PassFull);
        }
        pass.by_publisher.set(publisher.clone(), count + 1);
        pass.reads += 1;
        // La entrada ya vive hasta el vencimiento y la gracia (`buy_global_pass`).
        env.storage().persistent().set(&key, &pass);
        extend_instance_ttl(&env);

        PassRead {
            reader,
            publisher,
            paper_id,
            reads: pass.reads,
        }
        .publish(&env);

        pass.reads
    }

    /// Liquida el Pase PaperPay vencido de `reader`: la tesorería recibe la comisión y el resto se
    /// reparte entre las editoriales en proporción a las lecturas de cada una (lo que sobra por
    /// redondeo va a la primera). Si no leyó nada, todo vuelve a `reader`, sin comisión. Solo
    /// después de vencer (`PassActive` antes). No pide firmas: la puede enviar cualquiera (el
    /// sitio o una editorial), porque el dinero solo va a quienes fueron leídos o de vuelta a
    /// quien pagó. Quien no puede recibir queda con su parte pendiente para `claim`. Emite un
    /// `pass_share` por editorial y `pass_settled`; devuelve el reparto.
    pub fn settle_pass(env: Env, reader: Address) -> Split {
        let key = DataKey::GlobalPass(reader.clone());
        let pass = read_global_pass(&env, &key);
        if env.ledger().timestamp() < pass.until {
            panic_with_error!(&env, Error::PassActive);
        }
        env.storage().persistent().remove(&key);

        let config = read_config(&env);
        let split = if pass.reads == 0 {
            payout(&env, &config, &reader, pass.amount);
            Split {
                publisher: 0,
                fee: 0,
            }
        } else {
            let split = compute_split(&env, pass.amount, config.fee_bps);
            let mut parts: Vec<i128> = Vec::new(&env);
            let mut assigned: i128 = 0;
            for (_, count) in pass.by_publisher.iter() {
                let part = match split.publisher.checked_mul(count as i128) {
                    Some(scaled) => scaled / pass.reads as i128,
                    None => panic_with_error!(&env, Error::AmountOverflow),
                };
                // Cada parte es un piso: la suma nunca pasa de lo que hay para repartir.
                assigned += part;
                parts.push_back(part);
            }
            parts.set(0, parts.get_unchecked(0) + (split.publisher - assigned));
            for ((publisher, count), part) in pass.by_publisher.iter().zip(parts.iter()) {
                payout(&env, &config, &publisher, part);
                PassShare {
                    publisher,
                    reader: reader.clone(),
                    reads: count,
                    amount: part,
                }
                .publish(&env);
            }
            payout(&env, &config, &config.treasury, split.fee);
            split
        };
        extend_instance_ttl(&env);

        PassSettled {
            reader,
            amount: pass.amount,
            fee: split.fee,
            reads: pass.reads,
            publishers: pass.by_publisher.len(),
        }
        .publish(&env);

        split
    }

    /// `reader` cancela su Pase PaperPay y recupera todo lo que pagó. Solo mientras no tenga
    /// lecturas anotadas (`PassUsed` después). Solo firma `reader`. Emite `pass_cancelled` y
    /// devuelve lo recuperado.
    pub fn cancel_pass(env: Env, reader: Address) -> i128 {
        reader.require_auth();
        let key = DataKey::GlobalPass(reader.clone());
        let pass = read_global_pass(&env, &key);
        if pass.reads > 0 {
            panic_with_error!(&env, Error::PassUsed);
        }
        env.storage().persistent().remove(&key);

        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &env.current_contract_address(),
            &reader,
            &pass.amount,
        );
        extend_instance_ttl(&env);

        PassCancelled {
            reader,
            amount: pass.amount,
        }
        .publish(&env);

        pass.amount
    }

    /// El Pase PaperPay de `reader`, si tiene uno sin liquidar.
    pub fn global_pass(env: Env, reader: Address) -> Option<GlobalPass> {
        env.storage().persistent().get(&DataKey::GlobalPass(reader))
    }

    /// De soporte (no mueve fondos): `founder` crea un journal club con una cuota de `dues` por
    /// miembro y un quórum de `quorum` aprobaciones para pagar una lectura (de 2 a
    /// `MAX_CLUB_MEMBERS`). Nadie es miembro hasta que paga su cuota con `join_club`, tampoco
    /// quien lo crea. Solo firma `founder`. Emite `club_created` y devuelve el id del club.
    pub fn create_club(env: Env, founder: Address, dues: i128, quorum: u32) -> u32 {
        founder.require_auth();
        if dues <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        if !(2..=MAX_CLUB_MEMBERS).contains(&quorum) {
            panic_with_error!(&env, Error::InvalidQuorum);
        }
        let id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextClub)
            .unwrap_or(0);
        env.storage().instance().set(&DataKey::NextClub, &(id + 1));
        let key = DataKey::Club(id);
        env.storage().persistent().set(
            &key,
            &Club {
                dues,
                quorum,
                members: Vec::new(&env),
                balance: 0,
                proposals: 0,
            },
        );
        keep_max(&env, &key);
        extend_instance_ttl(&env);

        ClubCreated {
            founder,
            club_id: id,
            dues,
            quorum,
        }
        .publish(&env);

        id
    }

    /// `member` entra al club `club_id` y paga su cuota: pasa de su wallet a la caja del club, en
    /// el contrato. Hasta `MAX_CLUB_MEMBERS` miembros. Solo firma `member`. Emite `club_joined` y
    /// devuelve la caja del club.
    pub fn join_club(env: Env, member: Address, club_id: u32) -> i128 {
        member.require_auth();
        let key = DataKey::Club(club_id);
        let mut club = read_club(&env, &key);
        if club.members.contains(&member) {
            panic_with_error!(&env, Error::AlreadyMember);
        }
        if club.members.len() >= MAX_CLUB_MEMBERS {
            panic_with_error!(&env, Error::ClubFull);
        }
        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &member,
            env.current_contract_address(),
            &club.dues,
        );
        club.members.push_back(member.clone());
        club.balance = add(&env, club.balance, club.dues);
        env.storage().persistent().set(&key, &club);
        keep_max(&env, &key);
        extend_instance_ttl(&env);

        ClubJoined {
            member,
            club_id,
            dues: club.dues,
            balance: club.balance,
            members: club.members.len(),
        }
        .publish(&env);

        club.balance
    }

    /// De soporte (no mueve fondos): `member` propone que su club pague `amount` a `publisher` por
    /// `paper_id`. Tiene que alcanzar la caja (`ClubTooLow`). Su propia aprobación queda contada.
    /// Solo firma `member`. Emite `read_proposed` y devuelve el id de la propuesta.
    pub fn propose_read(
        env: Env,
        member: Address,
        club_id: u32,
        publisher: Address,
        paper_id: String,
        amount: i128,
    ) -> u32 {
        member.require_auth();
        validate_paper_id(&env, &paper_id);
        if amount <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        let key = DataKey::Club(club_id);
        let mut club = read_member_club(&env, &key, &member);
        if amount > club.balance {
            panic_with_error!(&env, Error::ClubTooLow);
        }
        let id = club.proposals;
        club.proposals += 1;
        env.storage().persistent().set(&key, &club);
        keep_max(&env, &key);
        let proposal_key = DataKey::Proposal(club_id, id);
        let mut approvals = Vec::new(&env);
        approvals.push_back(member.clone());
        env.storage().persistent().set(
            &proposal_key,
            &Proposal {
                publisher: publisher.clone(),
                paper_id: paper_id.clone(),
                amount,
                approvals,
                paid: false,
            },
        );
        keep_max(&env, &proposal_key);
        extend_instance_ttl(&env);

        ReadProposed {
            member,
            club_id,
            proposal_id: id,
            publisher,
            paper_id,
            amount,
        }
        .publish(&env);

        id
    }

    /// `member` aprueba la propuesta `proposal_id` de su club. Cuando las aprobaciones de miembros
    /// actuales llegan al quórum, esta misma llamada paga: `amount` sale de la caja, 98% a la
    /// editorial y 2% a la tesorería, y el artículo queda pagado para todos los miembros
    /// (`club_paid`). Cada miembro aprueba una vez; una propuesta se paga una vez. Si a la caja ya
    /// no le alcanza o la editorial no puede recibir, falla y la aprobación no queda anotada. Solo
    /// firma `member`. Emite `read_approved` (y `club_paid` si pagó); devuelve si pagó.
    pub fn approve_read(env: Env, member: Address, club_id: u32, proposal_id: u32) -> bool {
        member.require_auth();
        let key = DataKey::Club(club_id);
        let mut club = read_member_club(&env, &key, &member);
        let proposal_key = DataKey::Proposal(club_id, proposal_id);
        let mut proposal: Proposal = match env.storage().persistent().get(&proposal_key) {
            Some(proposal) => proposal,
            None => panic_with_error!(&env, Error::NoProposal),
        };
        if proposal.paid {
            panic_with_error!(&env, Error::ProposalClosed);
        }
        if proposal.approvals.contains(&member) {
            panic_with_error!(&env, Error::AlreadyApproved);
        }
        proposal.approvals.push_back(member.clone());
        // Solo cuentan las aprobaciones de quienes siguen en el club.
        let mut approvals: u32 = 0;
        for account in proposal.approvals.iter() {
            if club.members.contains(&account) {
                approvals += 1;
            }
        }
        let paid = approvals >= club.quorum;
        if paid {
            if proposal.amount > club.balance {
                panic_with_error!(&env, Error::ClubTooLow);
            }
            club.balance -= proposal.amount;
            proposal.paid = true;
            env.storage().persistent().set(&key, &club);
            let config = read_config(&env);
            let split = compute_split(&env, proposal.amount, config.fee_bps);
            transfer_split(
                &env,
                &config,
                &env.current_contract_address(),
                &proposal.publisher,
                &split,
            );
            ClubPaid {
                publisher: proposal.publisher.clone(),
                club_id,
                proposal_id,
                paper_id: proposal.paper_id.clone(),
                amount: proposal.amount,
                fee: split.fee,
                members: club.members.clone(),
            }
            .publish(&env);
        }
        env.storage().persistent().set(&proposal_key, &proposal);
        keep_max(&env, &proposal_key);
        keep_max(&env, &key);
        extend_instance_ttl(&env);

        ReadApproved {
            member,
            club_id,
            proposal_id,
            approvals,
            paid,
        }
        .publish(&env);

        paid
    }

    /// `member` sale del club `club_id` y se lleva una parte igual de lo que queda en la caja: la
    /// caja entre los miembros actuales, redondeada hacia abajo. Sus aprobaciones dejan de contar.
    /// El último en salir se lleva todo lo que queda. Solo firma `member`. Emite `club_left` y
    /// devuelve lo que se llevó.
    pub fn leave_club(env: Env, member: Address, club_id: u32) -> i128 {
        member.require_auth();
        let key = DataKey::Club(club_id);
        let mut club = read_member_club(&env, &key, &member);
        let share = club.balance / club.members.len() as i128;
        let index = club.members.first_index_of(&member).unwrap();
        club.members.remove(index);
        club.balance -= share;
        env.storage().persistent().set(&key, &club);
        keep_max(&env, &key);

        if share > 0 {
            let config = read_config(&env);
            token::Client::new(&env, &config.token).transfer(
                &env.current_contract_address(),
                &member,
                &share,
            );
        }
        extend_instance_ttl(&env);

        ClubLeft {
            member,
            club_id,
            amount: share,
            balance: club.balance,
        }
        .publish(&env);

        share
    }

    /// El journal club `club_id`, si existe.
    pub fn club(env: Env, club_id: u32) -> Option<Club> {
        env.storage().persistent().get(&DataKey::Club(club_id))
    }

    /// La propuesta `proposal_id` del club `club_id`, si existe.
    pub fn proposal(env: Env, club_id: u32, proposal_id: u32) -> Option<Proposal> {
        env.storage()
            .persistent()
            .get(&DataKey::Proposal(club_id, proposal_id))
    }

    /// `reader` paga `amount` por `paper_id` de `publisher`, pero contra entrega: el dinero pasa de
    /// su wallet al contrato y queda atado a `key_hash`, el sha256 de la llave que abre el
    /// artículo (la editorial le da ese hash junto con el artículo cifrado). Hasta `deadline`
    /// (segundos Unix, a no más de `MAX_LOCK_DAYS`) solo puede salir hacia la editorial, y solo
    /// si alguien presenta la llave (`unlock_payment`); desde `deadline`, solo puede volver a
    /// `reader` (`reclaim_payment`). Solo firma `reader`. Emite `payment_locked` y devuelve el id.
    pub fn lock_payment(
        env: Env,
        reader: Address,
        publisher: Address,
        paper_id: String,
        amount: i128,
        key_hash: BytesN<32>,
        deadline: u64,
    ) -> u32 {
        reader.require_auth();
        validate_paper_id(&env, &paper_id);
        if amount <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        let now = env.ledger().timestamp();
        if deadline <= now || deadline - now > MAX_LOCK_DAYS as u64 * DAY_SECONDS {
            panic_with_error!(&env, Error::InvalidDeadline);
        }

        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &reader,
            env.current_contract_address(),
            &amount,
        );
        let id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextLock)
            .unwrap_or(0);
        env.storage().instance().set(&DataKey::NextLock, &(id + 1));
        let entry = DataKey::Lock(id);
        env.storage().persistent().set(
            &entry,
            &Lock {
                reader: reader.clone(),
                publisher: publisher.clone(),
                paper_id: paper_id.clone(),
                amount,
                key_hash: key_hash.clone(),
                deadline,
                open: true,
            },
        );
        keep_campaign(&env, &entry, deadline);
        extend_instance_ttl(&env);

        PaymentLocked {
            reader,
            publisher,
            lock_id: id,
            paper_id,
            amount,
            key_hash,
            deadline,
        }
        .publish(&env);

        id
    }

    /// Libera el pago contra entrega `lock_id` con `key`, la llave que abre el artículo: si su
    /// sha256 es el pactado y el plazo no venció, la editorial recibe el monto menos la comisión y
    /// la tesorería, la comisión. No pide firma: la llave es la prueba de la entrega, y el dinero
    /// solo puede ir a la editorial del pago, la envíe quien la envíe. La llave queda publicada en
    /// `payment_unlocked`: así la recibe quien pagó, en la misma transacción en que la editorial
    /// cobra. Si la editorial no puede recibir (sin trustline), su parte queda pendiente a su
    /// nombre para `claim`. Emite `paid` y `payment_unlocked`; devuelve el reparto.
    pub fn unlock_payment(env: Env, lock_id: u32, key: BytesN<32>) -> Split {
        let entry = DataKey::Lock(lock_id);
        let mut lock = read_lock(&env, &entry);
        if env.ledger().timestamp() >= lock.deadline {
            panic_with_error!(&env, Error::LockExpired);
        }
        if env.crypto().sha256(&key.clone().into()).to_bytes() != lock.key_hash {
            panic_with_error!(&env, Error::WrongKey);
        }
        lock.open = false;
        env.storage().persistent().set(&entry, &lock);

        let config = read_config(&env);
        let split = compute_split(&env, lock.amount, config.fee_bps);
        payout(&env, &config, &lock.publisher, split.publisher);
        payout(&env, &config, &config.treasury, split.fee);
        extend_instance_ttl(&env);

        Paid {
            publisher: lock.publisher.clone(),
            reader: lock.reader.clone(),
            paper_id: lock.paper_id,
            payer: lock.reader.clone(),
            amount: lock.amount,
            fee: split.fee,
        }
        .publish(&env);
        PaymentUnlocked {
            reader: lock.reader,
            publisher: lock.publisher,
            lock_id,
            key,
        }
        .publish(&env);

        split
    }

    /// `reader` recupera su pago contra entrega `lock_id` completo, sin comisión, porque la llave
    /// no llegó: solo desde el plazo (`LockActive` antes) y si no se cobró. Solo firma `reader`.
    /// Emite `payment_reclaimed` y devuelve lo recuperado.
    pub fn reclaim_payment(env: Env, reader: Address, lock_id: u32) -> i128 {
        reader.require_auth();
        let entry = DataKey::Lock(lock_id);
        let mut lock = read_lock(&env, &entry);
        if lock.reader != reader {
            panic_with_error!(&env, Error::NoLock);
        }
        if env.ledger().timestamp() < lock.deadline {
            panic_with_error!(&env, Error::LockActive);
        }
        lock.open = false;
        env.storage().persistent().set(&entry, &lock);

        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &env.current_contract_address(),
            &reader,
            &lock.amount,
        );
        extend_instance_ttl(&env);

        PaymentReclaimed {
            reader,
            lock_id,
            amount: lock.amount,
        }
        .publish(&env);

        lock.amount
    }

    /// El pago contra entrega `lock_id`, si existe.
    pub fn lock(env: Env, lock_id: u32) -> Option<Lock> {
        env.storage().persistent().get(&DataKey::Lock(lock_id))
    }

    /// `reviewer` toma la recompensa `bounty_id` y deja `stake` en garantía: pasa de su wallet al
    /// contrato. La garantía es al menos el `MIN_STAKE_BPS` (10%) de la recompensa
    /// (`StakeTooLow`). Solo se toma una recompensa abierta y antes de su plazo, una vez por
    /// cuenta; varias cuentas pueden tomar la misma. La garantía tiene dos salidas: vuelve a
    /// `reviewer` si entrega antes del plazo (`submit_review`) o, si no, pasa a quien publicó la
    /// recompensa (`forfeit_stake`). Solo firma `reviewer`. Emite `bounty_accepted` y devuelve el
    /// plazo de entrega (segundos Unix).
    pub fn accept_bounty(env: Env, reviewer: Address, bounty_id: u32, stake: i128) -> u64 {
        reviewer.require_auth();
        let bounty = read_any_bounty(&env, bounty_id);
        if !bounty.open {
            panic_with_error!(&env, Error::BountyClosed);
        }
        if env.ledger().timestamp() >= bounty.deadline {
            panic_with_error!(&env, Error::BountyExpired);
        }
        if stake <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        let minimum = match bounty.amount.checked_mul(MIN_STAKE_BPS) {
            Some(scaled) => scaled / BPS_DENOMINATOR,
            None => panic_with_error!(&env, Error::AmountOverflow),
        };
        if stake < minimum {
            panic_with_error!(&env, Error::StakeTooLow);
        }
        let entry = DataKey::Stake(bounty_id, reviewer.clone());
        if env.storage().persistent().has(&entry) {
            panic_with_error!(&env, Error::AlreadyAccepted);
        }

        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &reviewer,
            env.current_contract_address(),
            &stake,
        );
        env.storage().persistent().set(&entry, &stake);
        keep_campaign(&env, &entry, bounty.deadline);
        extend_instance_ttl(&env);

        BountyAccepted {
            reviewer,
            bounty_id,
            stake,
            deadline: bounty.deadline,
        }
        .publish(&env);

        bounty.deadline
    }

    /// `reviewer` entrega su trabajo para la recompensa `bounty_id`: `report_hash` es el sha256 de
    /// su informe, que queda en la red con la hora de entrega. Si es antes del plazo
    /// (`BountyExpired` después), recupera su garantía completa. La entrega vale aunque la
    /// recompensa ya se haya repartido: la garantía no queda atrapada. El contrato comprueba que
    /// hubo una entrega a tiempo, no su calidad: eso lo juzga quien publicó la recompensa al
    /// repartirla. Solo firma `reviewer`. Emite `review_submitted` y devuelve la garantía.
    pub fn submit_review(
        env: Env,
        reviewer: Address,
        bounty_id: u32,
        report_hash: BytesN<32>,
    ) -> i128 {
        reviewer.require_auth();
        let entry = DataKey::Stake(bounty_id, reviewer.clone());
        let stake = read_stake(&env, &entry);
        if env.ledger().timestamp() >= read_any_bounty(&env, bounty_id).deadline {
            panic_with_error!(&env, Error::BountyExpired);
        }
        env.storage().persistent().remove(&entry);

        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &env.current_contract_address(),
            &reviewer,
            &stake,
        );
        extend_instance_ttl(&env);

        ReviewSubmitted {
            reviewer,
            bounty_id,
            report_hash,
            stake,
        }
        .publish(&env);

        stake
    }

    /// Ejecuta la garantía de `reviewer` en la recompensa `bounty_id`: si llegó el plazo
    /// (`BountyActive` antes) y no entregó, la garantía pasa completa, sin comisión, a quien
    /// publicó la recompensa. No pide firma: la ejecuta cualquiera y el dinero solo puede ir a
    /// esa cuenta. Si no puede recibir (sin trustline), queda pendiente a su nombre para `claim`.
    /// Emite `stake_forfeited` y devuelve la garantía.
    pub fn forfeit_stake(env: Env, bounty_id: u32, reviewer: Address) -> i128 {
        let entry = DataKey::Stake(bounty_id, reviewer.clone());
        let stake = read_stake(&env, &entry);
        let bounty = read_any_bounty(&env, bounty_id);
        if env.ledger().timestamp() < bounty.deadline {
            panic_with_error!(&env, Error::BountyActive);
        }
        env.storage().persistent().remove(&entry);

        let config = read_config(&env);
        payout(&env, &config, &bounty.funder, stake);
        extend_instance_ttl(&env);

        StakeForfeited {
            reviewer,
            funder: bounty.funder,
            bounty_id,
            amount: stake,
        }
        .publish(&env);

        stake
    }

    /// La garantía de `reviewer` en la recompensa `bounty_id` (0 si no tiene).
    pub fn stake(env: Env, bounty_id: u32, reviewer: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Stake(bounty_id, reviewer))
            .unwrap_or(0)
    }

    /// `funder` encarga a `translator` la traducción de `paper_id` (o su resumen de divulgación)
    /// y deja el pago completo en el contrato, dividido en `milestones` (de 1 a
    /// `MAX_MILESTONES` montos, por ejemplo borrador, revisión y versión final). Hasta
    /// `deadline` (segundos Unix, a no más de `MAX_GRANT_DAYS`) ese dinero solo puede salir
    /// hacia `translator`, hito por hito (`release_milestone`); después, lo no liberado puede
    /// volver a `funder` (`cancel_grant`). Solo firma `funder`. Emite `grant_funded` y devuelve el
    /// id del encargo.
    pub fn fund_grant(
        env: Env,
        funder: Address,
        translator: Address,
        paper_id: String,
        milestones: Vec<i128>,
        deadline: u64,
    ) -> u32 {
        funder.require_auth();
        validate_paper_id(&env, &paper_id);
        if milestones.is_empty() || milestones.len() > MAX_MILESTONES {
            panic_with_error!(&env, Error::InvalidMilestones);
        }
        let mut total: i128 = 0;
        for milestone in milestones.iter() {
            if milestone <= 0 {
                panic_with_error!(&env, Error::InvalidMilestones);
            }
            total = add(&env, total, milestone);
        }
        let now = env.ledger().timestamp();
        if deadline <= now || deadline - now > MAX_GRANT_DAYS as u64 * DAY_SECONDS {
            panic_with_error!(&env, Error::InvalidDeadline);
        }

        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &funder,
            env.current_contract_address(),
            &total,
        );
        let id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextGrant)
            .unwrap_or(0);
        env.storage().instance().set(&DataKey::NextGrant, &(id + 1));
        let entry = DataKey::Grant(id);
        let count = milestones.len();
        env.storage().persistent().set(
            &entry,
            &Grant {
                funder: funder.clone(),
                translator: translator.clone(),
                paper_id: paper_id.clone(),
                milestones,
                released: 0,
                deadline,
                open: true,
            },
        );
        keep_campaign(&env, &entry, deadline);
        extend_instance_ttl(&env);

        GrantFunded {
            funder,
            translator,
            grant_id: id,
            paper_id,
            total,
            milestones: count,
            deadline,
        }
        .publish(&env);

        id
    }

    /// `funder` libera el siguiente hito de su encargo `grant_id`: quien traduce recibe el hito
    /// menos la comisión y la tesorería, la comisión. Los hitos salen en orden, uno por llamada;
    /// el último cierra el encargo. Se puede liberar antes o después del plazo, mientras no se
    /// haya cancelado. Si quien traduce no puede recibir (sin trustline), su parte queda
    /// pendiente a su nombre para `claim`. Solo firma `funder`. Emite `milestone_released` y
    /// devuelve el reparto del hito.
    pub fn release_milestone(env: Env, funder: Address, grant_id: u32) -> Split {
        funder.require_auth();
        let entry = DataKey::Grant(grant_id);
        let mut grant = read_grant(&env, &entry, &funder);
        let index = grant.released;
        let amount = grant.milestones.get_unchecked(index);
        grant.released = index + 1;
        let remaining = grant.milestones.len() - grant.released;
        if remaining == 0 {
            grant.open = false;
        }
        env.storage().persistent().set(&entry, &grant);

        let config = read_config(&env);
        let split = compute_split(&env, amount, config.fee_bps);
        payout(&env, &config, &grant.translator, split.publisher);
        payout(&env, &config, &config.treasury, split.fee);
        extend_instance_ttl(&env);

        MilestoneReleased {
            translator: grant.translator,
            funder,
            grant_id,
            milestone: index,
            amount,
            fee: split.fee,
            remaining,
        }
        .publish(&env);

        split
    }

    /// `funder` cancela su encargo `grant_id` y recupera los hitos que no liberó, sin comisión.
    /// Solo desde el plazo (`GrantActive` antes): mientras tanto, quien traduce sabe que el
    /// dinero está ahí. Lo ya pagado no vuelve. Solo firma `funder`. Emite `grant_cancelled` y
    /// devuelve lo recuperado.
    pub fn cancel_grant(env: Env, funder: Address, grant_id: u32) -> i128 {
        funder.require_auth();
        let entry = DataKey::Grant(grant_id);
        let mut grant = read_grant(&env, &entry, &funder);
        if env.ledger().timestamp() < grant.deadline {
            panic_with_error!(&env, Error::GrantActive);
        }
        let mut unreleased: i128 = 0;
        for index in grant.released..grant.milestones.len() {
            // La suma completa ya pasó por `add` al encargar: una parte no se desborda.
            unreleased += grant.milestones.get_unchecked(index);
        }
        grant.open = false;
        env.storage().persistent().set(&entry, &grant);

        let config = read_config(&env);
        token::Client::new(&env, &config.token).transfer(
            &env.current_contract_address(),
            &funder,
            &unreleased,
        );
        extend_instance_ttl(&env);

        GrantCancelled {
            funder,
            grant_id,
            amount: unreleased,
        }
        .publish(&env);

        unreleased
    }

    /// El encargo `grant_id`, si existe.
    pub fn grant(env: Env, grant_id: u32) -> Option<Grant> {
        env.storage().persistent().get(&DataKey::Grant(grant_id))
    }

    /// De soporte, no mueve fondos: `owner` autoriza a `agent` (por ejemplo, un agente de IA que
    /// investiga por él) a pagar lecturas con su USDC: hasta `max_per_read` por lectura, `budget`
    /// en total y durante `days` días (de 1 a `MAX_AGENT_DAYS`). El presupuesto es una
    /// autorización del token al contrato (allowance): el dinero sigue en la wallet de `owner`
    /// hasta cada pago. Un permiso nuevo reemplaza al anterior, con su presupuesto. No se puede
    /// con una suscripción activa (`AllowanceInUse`): comparten la autorización del token. Solo
    /// firma `owner`. Emite `agent_authorized` y devuelve cuándo vence el permiso.
    pub fn authorize_agent(
        env: Env,
        owner: Address,
        agent: Address,
        max_per_read: i128,
        budget: i128,
        days: u32,
    ) -> u64 {
        owner.require_auth();
        if max_per_read <= 0 || budget < max_per_read {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        if days == 0 || days > MAX_AGENT_DAYS {
            panic_with_error!(&env, Error::InvalidDays);
        }
        if env.storage().persistent().has(&DataKey::Sub(owner.clone())) {
            panic_with_error!(&env, Error::AllowanceInUse);
        }

        let config = read_config(&env);
        token::Client::new(&env, &config.token).approve(
            &owner,
            &env.current_contract_address(),
            &budget,
            &allowance_until(&env, days),
        );
        let expires_at = env.ledger().timestamp() + days as u64 * DAY_SECONDS;
        let entry = DataKey::Agent(owner.clone());
        env.storage().persistent().set(
            &entry,
            &AgentPermit {
                agent: agent.clone(),
                max_per_read,
                expires_at,
            },
        );
        keep_until(&env, &entry, expires_at);
        extend_instance_ttl(&env);

        AgentAuthorized {
            owner,
            agent,
            max_per_read,
            budget,
            expires_at,
        }
        .publish(&env);

        expires_at
    }

    /// `agent` paga `amount` por `paper_id` de `publisher` con el USDC de `owner`, que lo
    /// autorizó (`authorize_agent`): la editorial recibe el monto menos la comisión y la
    /// tesorería, la comisión, directo desde la wallet de `owner`. Solo firma `agent`: ni el
    /// dueño ni la editorial. El contrato exige que sea el agente de ese dueño (`NoAgent`), que
    /// el permiso no haya vencido (`AgentExpired`), que el monto no pase del tope por lectura
    /// (`AboveMaxPerRead`) ni de lo que queda del presupuesto (`BudgetExhausted`). El acceso
    /// queda a nombre de `owner`. Emite `paid` y `agent_paid`; devuelve el reparto.
    pub fn pay_as_agent(
        env: Env,
        agent: Address,
        owner: Address,
        publisher: Address,
        paper_id: String,
        amount: i128,
    ) -> Split {
        agent.require_auth();
        let permit = read_permit(&env, &owner);
        if permit.agent != agent {
            panic_with_error!(&env, Error::NoAgent);
        }
        if env.ledger().timestamp() >= permit.expires_at {
            panic_with_error!(&env, Error::AgentExpired);
        }
        if env.storage().persistent().has(&DataKey::Sub(owner.clone())) {
            panic_with_error!(&env, Error::AllowanceInUse);
        }
        validate_paper_id(&env, &paper_id);
        let config = read_config(&env);
        let split = compute_split(&env, amount, config.fee_bps);
        if amount > permit.max_per_read {
            panic_with_error!(&env, Error::AboveMaxPerRead);
        }
        let usdc = token::Client::new(&env, &config.token);
        let spender = env.current_contract_address();
        let budget = usdc.allowance(&owner, &spender);
        if amount > budget {
            panic_with_error!(&env, Error::BudgetExhausted);
        }

        if split.publisher > 0 {
            usdc.transfer_from(&spender, &owner, &publisher, &split.publisher);
        }
        if split.fee > 0 {
            usdc.transfer_from(&spender, &owner, &config.treasury, &split.fee);
        }
        extend_instance_ttl(&env);

        Paid {
            publisher,
            reader: owner.clone(),
            paper_id: paper_id.clone(),
            payer: owner.clone(),
            amount,
            fee: split.fee,
        }
        .publish(&env);
        AgentPaid {
            owner,
            agent,
            paper_id,
            amount,
            remaining: budget - amount,
        }
        .publish(&env);

        split
    }

    /// De soporte, no mueve fondos: `owner` retira el permiso de su agente y deja en cero la
    /// autorización del token (si hoy es de una suscripción, no la toca). Solo firma `owner`.
    /// Emite `agent_revoked`.
    pub fn revoke_agent(env: Env, owner: Address) {
        owner.require_auth();
        let permit = read_permit(&env, &owner);
        env.storage()
            .persistent()
            .remove(&DataKey::Agent(owner.clone()));
        if !env.storage().persistent().has(&DataKey::Sub(owner.clone())) {
            let config = read_config(&env);
            // Monto 0 con vencimiento 0, como al cancelar una suscripción.
            token::Client::new(&env, &config.token).approve(
                &owner,
                &env.current_contract_address(),
                &0,
                &0,
            );
        }
        extend_instance_ttl(&env);

        AgentRevoked {
            owner,
            agent: permit.agent,
        }
        .publish(&env);
    }

    /// El permiso del agente de `owner`, si tiene uno.
    pub fn agent(env: Env, owner: Address) -> Option<AgentPermit> {
        env.storage().persistent().get(&DataKey::Agent(owner))
    }

    /// `account` paga `amount` por `paper_id` de `publisher` con lo que el contrato le debe (una
    /// recompensa, un hito o un cobro que quedó pendiente porque no podía recibir), sin retirarlo
    /// antes: la editorial recibe el monto menos la comisión y la tesorería, la comisión, desde
    /// la custodia del contrato. Sirve a quien todavía no tiene trustline de USDC: lee con lo que
    /// ganó. Si lo pendiente no alcanza, falla (`OwedTooLow`) y no cambia nada. Si la editorial no
    /// puede recibir, su parte queda pendiente a su nombre. Solo firma `account`. Emite `paid` y
    /// `paid_from_owed`; devuelve el reparto.
    pub fn pay_from_owed(
        env: Env,
        account: Address,
        publisher: Address,
        paper_id: String,
        amount: i128,
    ) -> Split {
        account.require_auth();
        validate_paper_id(&env, &paper_id);
        let config = read_config(&env);
        let split = compute_split(&env, amount, config.fee_bps);
        let entry = DataKey::Owed(account.clone());
        let owed: i128 = env.storage().persistent().get(&entry).unwrap_or(0);
        if amount > owed {
            panic_with_error!(&env, Error::OwedTooLow);
        }
        let remaining = owed - amount;
        if remaining == 0 {
            env.storage().persistent().remove(&entry);
        } else {
            env.storage().persistent().set(&entry, &remaining);
            keep_max(&env, &entry);
        }

        payout(&env, &config, &publisher, split.publisher);
        payout(&env, &config, &config.treasury, split.fee);
        extend_instance_ttl(&env);

        Paid {
            publisher,
            reader: account.clone(),
            paper_id: paper_id.clone(),
            payer: account.clone(),
            amount,
            fee: split.fee,
        }
        .publish(&env);
        PaidFromOwed {
            account,
            paper_id,
            amount,
            remaining,
        }
        .publish(&env);

        split
    }

    /// `reader` renta `paper_id` de `publisher` por `period` segundos (de 1 a `MAX_RENT_SECONDS`):
    /// paga `amount`, que va directo de su wallet a la editorial (menos la comisión) y a la
    /// tesorería. `full_price` es el precio de compra del artículo y tiene que ser mayor que la
    /// renta (`RentNotCheaper`): el contrato lo guarda junto con lo pagado, para que
    /// `buy_out` cobre solo la diferencia mientras la renta siga vigente. Una renta vigente no se
    /// repite (`RentalActive`); una vencida se reemplaza. Como en `pay`, los precios los valida
    /// quien da el acceso, contra el evento. Solo firma `reader`. Emite `rented` y devuelve hasta
    /// cuándo vale la renta (segundos Unix).
    pub fn rent(
        env: Env,
        reader: Address,
        publisher: Address,
        paper_id: String,
        amount: i128,
        full_price: i128,
        period: u64,
    ) -> u64 {
        reader.require_auth();
        validate_paper_id(&env, &paper_id);
        if period == 0 || period > MAX_RENT_SECONDS {
            panic_with_error!(&env, Error::InvalidDays);
        }
        let config = read_config(&env);
        let split = compute_split(&env, amount, config.fee_bps);
        if full_price <= amount {
            panic_with_error!(&env, Error::RentNotCheaper);
        }
        let entry = DataKey::Rental(reader.clone(), publisher.clone(), paper_id.clone());
        let now = env.ledger().timestamp();
        let current: Option<Rental> = env.storage().persistent().get(&entry);
        if let Some(rental) = current {
            if now < rental.until {
                panic_with_error!(&env, Error::RentalActive);
            }
        }

        transfer_split(&env, &config, &reader, &publisher, &split);
        let until = now + period;
        env.storage().persistent().set(
            &entry,
            &Rental {
                paid: amount,
                full_price,
                until,
            },
        );
        keep_until(&env, &entry, until);
        extend_instance_ttl(&env);

        Rented {
            publisher,
            reader,
            paper_id,
            amount,
            fee: split.fee,
            full_price,
            until,
        }
        .publish(&env);

        until
    }

    /// `reader` convierte su renta vigente de `paper_id` de `publisher` en compra: paga el precio
    /// pactado al rentar menos lo que ya pagó de renta, con el mismo reparto. Solo mientras la
    /// renta no haya vencido (`RentalExpired` después); una renta se convierte una sola vez.
    /// Solo firma `reader`. Emite `paid` (el acceso ya no vence) y `bought_out`; devuelve el
    /// reparto de la diferencia.
    pub fn buy_out(env: Env, reader: Address, publisher: Address, paper_id: String) -> Split {
        reader.require_auth();
        let entry = DataKey::Rental(reader.clone(), publisher.clone(), paper_id.clone());
        let rental: Rental = match env.storage().persistent().get(&entry) {
            Some(rental) => rental,
            None => panic_with_error!(&env, Error::NoRental),
        };
        if env.ledger().timestamp() >= rental.until {
            panic_with_error!(&env, Error::RentalExpired);
        }
        env.storage().persistent().remove(&entry);

        let config = read_config(&env);
        // `full_price > paid` desde `rent`: la diferencia es positiva.
        let amount = rental.full_price - rental.paid;
        let split = compute_split(&env, amount, config.fee_bps);
        transfer_split(&env, &config, &reader, &publisher, &split);
        extend_instance_ttl(&env);

        Paid {
            publisher: publisher.clone(),
            reader: reader.clone(),
            paper_id: paper_id.clone(),
            payer: reader.clone(),
            amount,
            fee: split.fee,
        }
        .publish(&env);
        BoughtOut {
            publisher,
            reader,
            paper_id,
            rented: rental.paid,
            amount,
            total: rental.full_price,
        }
        .publish(&env);

        split
    }

    /// La renta de `reader` del artículo `paper_id` de `publisher`, si tiene una (vigente o
    /// vencida).
    pub fn rental(
        env: Env,
        reader: Address,
        publisher: Address,
        paper_id: String,
    ) -> Option<Rental> {
        env.storage()
            .persistent()
            .get(&DataKey::Rental(reader, publisher, paper_id))
    }

    /// Lo que el contrato le debe a `account` (pagos que no pudo recibir).
    pub fn owed(env: Env, account: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Owed(account))
            .unwrap_or(0)
    }

    /// Lo que aportó `backer` a la campaña `campaign_id` y sigue en garantía.
    pub fn pledge_of(env: Env, campaign_id: u32, backer: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Pledge(campaign_id, backer))
            .unwrap_or(0)
    }

    /// La campaña `campaign_id`, si existe.
    pub fn campaign(env: Env, campaign_id: u32) -> Option<Campaign> {
        env.storage()
            .persistent()
            .get(&DataKey::Campaign(campaign_id))
    }

    /// Cómo se repartiría `amount`, sin mover fondos.
    pub fn split(env: Env, amount: i128) -> Split {
        let config = read_config(&env);
        compute_split(&env, amount, config.fee_bps)
    }

    pub fn config(env: Env) -> Config {
        read_config(&env)
    }
}

/// Un cobro individual (`pay` y `gift`): reparte, extiende el TTL y emite el comprobante.
fn charge(
    env: &Env,
    payer: &Address,
    reader: &Address,
    paper_id: String,
    publisher: Address,
    amount: i128,
) -> Split {
    validate_paper_id(env, &paper_id);
    let config = read_config(env);
    let split = compute_split(env, amount, config.fee_bps);

    transfer_split(env, &config, payer, &publisher, &split);
    extend_instance_ttl(env);

    Paid {
        publisher,
        reader: reader.clone(),
        paper_id,
        payer: payer.clone(),
        amount,
        fee: split.fee,
    }
    .publish(env);

    split
}

/// Varios cobros con una firma (`pay_batch` y `gift_batch`): valida todo antes de mover fondos,
/// agrupa las transferencias por editorial y emite un `paid` por artículo, a nombre de `reader`.
fn charge_batch(env: &Env, payer: &Address, reader: &Address, items: Vec<Item>) -> Split {
    if items.is_empty() {
        panic_with_error!(env, Error::EmptyBatch);
    }
    if items.len() > MAX_BATCH_ITEMS {
        panic_with_error!(env, Error::BatchTooLarge);
    }

    let config = read_config(env);
    let mut seen: Map<String, bool> = Map::new(env);
    let mut per_publisher: Map<Address, i128> = Map::new(env);
    let mut fees: Vec<i128> = Vec::new(env);
    let mut total = Split {
        publisher: 0,
        fee: 0,
    };

    for item in items.iter() {
        validate_paper_id(env, &item.paper_id);
        if seen.contains_key(item.paper_id.clone()) {
            panic_with_error!(env, Error::DuplicatePaper);
        }
        seen.set(item.paper_id.clone(), true);

        let split = compute_split(env, item.amount, config.fee_bps);
        let owed = per_publisher.get(item.publisher.clone()).unwrap_or(0);
        per_publisher.set(item.publisher.clone(), add(env, owed, split.publisher));
        total.publisher = add(env, total.publisher, split.publisher);
        total.fee = add(env, total.fee, split.fee);
        fees.push_back(split.fee);
    }

    let usdc = token::Client::new(env, &config.token);
    for (publisher, owed) in per_publisher.iter() {
        if owed > 0 {
            usdc.transfer(payer, &publisher, &owed);
        }
    }
    if total.fee > 0 {
        usdc.transfer(payer, &config.treasury, &total.fee);
    }
    extend_instance_ttl(env);

    for (item, fee) in items.iter().zip(fees.iter()) {
        Paid {
            publisher: item.publisher,
            reader: reader.clone(),
            paper_id: item.paper_id,
            payer: payer.clone(),
            amount: item.amount,
            fee,
        }
        .publish(env);
    }

    total
}

/// Mueve un reparto de `payer` a la editorial y a la tesorería (omite montos en cero). `payer` es
/// quien firmó o, en `pay_sponsored`, el propio contrato.
fn transfer_split(env: &Env, config: &Config, payer: &Address, publisher: &Address, split: &Split) {
    let usdc = token::Client::new(env, &config.token);
    if split.publisher > 0 {
        usdc.transfer(payer, publisher, &split.publisher);
    }
    if split.fee > 0 {
        usdc.transfer(payer, &config.treasury, &split.fee);
    }
}

/// Suma `days` días al pase de `reader` con `publisher` y devuelve el vencimiento nuevo.
fn grant_pass(env: &Env, reader: &Address, publisher: &Address, days: u32) -> u64 {
    extend_access(env, &DataKey::Pass(reader.clone(), publisher.clone()), days)
}

/// Suma `days` días a un acceso con vencimiento: desde el vencimiento vigente si sigue en pie, o
/// desde ahora si ya venció. Lo guarda con un TTL que lo cubre y devuelve el vencimiento nuevo.
fn extend_access(env: &Env, key: &DataKey, days: u32) -> u64 {
    let now = env.ledger().timestamp();
    let current: u64 = env.storage().persistent().get(key).unwrap_or(0);
    let until = current.max(now) + days as u64 * DAY_SECONDS;
    if until - now > MAX_ACCESS_DAYS as u64 * DAY_SECONDS {
        panic_with_error!(env, Error::TooLong);
    }
    env.storage().persistent().set(key, &until);
    keep_until(env, key, until);
    until
}

/// Extiende el TTL de una entrada persistente para que siga en la red hasta `until` (segundos
/// Unix) y `TTL_MARGIN_LEDGERS` más, sin pasar del máximo de la red.
fn keep_until(env: &Env, key: &DataKey, until: u64) {
    let secs = until.saturating_sub(env.ledger().timestamp());
    let ledgers = (secs / LEDGER_SECONDS) as u32 + TTL_MARGIN_LEDGERS;
    let ttl = ledgers.min(env.storage().max_ttl());
    env.storage().persistent().extend_ttl(key, ttl, ttl);
}

/// Paga `amount` desde la custodia del contrato. Si `to` no puede recibir (sin trustline, cuenta
/// inexistente o desautorizada), no tumba la operación: lo deja pendiente a su nombre y emite
/// `credited`. Devuelve si pagó.
fn payout(env: &Env, config: &Config, to: &Address, amount: i128) -> bool {
    if amount <= 0 {
        return true;
    }
    let usdc = token::Client::new(env, &config.token);
    match usdc.try_transfer(&env.current_contract_address(), to, &amount) {
        Ok(Ok(())) => true,
        _ => {
            credit(env, to, amount);
            false
        }
    }
}

/// Suma `amount` a lo que el contrato le debe a `to`. La entrada no tiene vencimiento: se lleva al
/// TTL máximo en cada abono (y si se archiva, se restaura con el saldo intacto).
fn credit(env: &Env, to: &Address, amount: i128) {
    let key = DataKey::Owed(to.clone());
    let owed: i128 = env.storage().persistent().get(&key).unwrap_or(0);
    env.storage()
        .persistent()
        .set(&key, &add(env, owed, amount));
    keep_max(env, &key);
    Credited {
        account: to.clone(),
        amount,
    }
    .publish(env);
}

/// El pago contra entrega de `entry`, si existe y sigue sin cobrarse ni devolverse.
fn read_lock(env: &Env, entry: &DataKey) -> Lock {
    let lock: Lock = match env.storage().persistent().get(entry) {
        Some(lock) => lock,
        None => panic_with_error!(env, Error::NoLock),
    };
    if !lock.open {
        panic_with_error!(env, Error::LockClosed);
    }
    lock
}

fn read_club(env: &Env, key: &DataKey) -> Club {
    match env.storage().persistent().get(key) {
        Some(club) => club,
        None => panic_with_error!(env, Error::NoClub),
    }
}

/// El club de `key`, si existe y `member` es miembro.
fn read_member_club(env: &Env, key: &DataKey, member: &Address) -> Club {
    let club = read_club(env, key);
    if !club.members.contains(member) {
        panic_with_error!(env, Error::NotMember);
    }
    club
}

fn read_global_pass(env: &Env, key: &DataKey) -> GlobalPass {
    match env.storage().persistent().get(key) {
        Some(pass) => pass,
        None => panic_with_error!(env, Error::NoGlobalPass),
    }
}

fn read_tab(env: &Env, key: &DataKey) -> Tab {
    match env.storage().persistent().get(key) {
        Some(tab) => tab,
        None => panic_with_error!(env, Error::NoTab),
    }
}

/// Paga desde el contrato todas las deudas de una cuenta abierta (el dinero ya está en el
/// contrato: lo acaba de mandar quien lee o es su garantía) y la deja sin deuda. Cada editorial
/// recibe lo suyo menos la comisión; la tesorería, la suma de las comisiones. Devuelve el reparto
/// total y cuántas editoriales cobraron.
fn pay_debts(env: &Env, config: &Config, tab: &mut Tab) -> (Split, u32) {
    let mut fee: i128 = 0;
    let publishers = tab.debts.len();
    for (publisher, debt) in tab.debts.iter() {
        let split = compute_split(env, debt, config.fee_bps);
        payout(env, config, &publisher, split.publisher);
        fee += split.fee;
    }
    payout(env, config, &config.treasury, fee);
    let split = Split {
        publisher: tab.owed - fee,
        fee,
    };
    tab.owed = 0;
    tab.due = 0;
    tab.debts = Map::new(env);
    (split, publishers)
}

/// El vale de `entry`, si existe y sigue sin usar.
fn read_voucher(env: &Env, entry: &DataKey) -> Voucher {
    let voucher: Voucher = match env.storage().persistent().get(entry) {
        Some(voucher) => voucher,
        None => panic_with_error!(env, Error::NoVoucher),
    };
    if !voucher.open {
        panic_with_error!(env, Error::VoucherSpent);
    }
    voucher
}

/// Una lista de llaves de vales: de 1 a `MAX_BATCH_ITEMS`.
fn check_keys(env: &Env, keys: &Vec<BytesN<32>>) {
    if keys.is_empty() {
        panic_with_error!(env, Error::EmptyBatch);
    }
    if keys.len() > MAX_BATCH_ITEMS {
        panic_with_error!(env, Error::BatchTooLarge);
    }
}

/// El permiso del agente de `owner`, si tiene uno.
fn read_permit(env: &Env, owner: &Address) -> AgentPermit {
    match env
        .storage()
        .persistent()
        .get(&DataKey::Agent(owner.clone()))
    {
        Some(permit) => permit,
        None => panic_with_error!(env, Error::NoAgent),
    }
}

/// El encargo de `entry`, si existe, es de `funder` y sigue abierto.
fn read_grant(env: &Env, entry: &DataKey, funder: &Address) -> Grant {
    let grant: Grant = match env.storage().persistent().get(entry) {
        Some(grant) => grant,
        None => panic_with_error!(env, Error::NoGrant),
    };
    if grant.funder != *funder {
        panic_with_error!(env, Error::NoGrant);
    }
    if !grant.open {
        panic_with_error!(env, Error::GrantClosed);
    }
    grant
}

/// La recompensa `bounty_id`, si existe (abierta o no, de quien sea).
fn read_any_bounty(env: &Env, bounty_id: u32) -> Bounty {
    match env.storage().persistent().get(&DataKey::Bounty(bounty_id)) {
        Some(bounty) => bounty,
        None => panic_with_error!(env, Error::NoBounty),
    }
}

/// La garantía de `entry`, si sigue en el contrato.
fn read_stake(env: &Env, entry: &DataKey) -> i128 {
    match env.storage().persistent().get(entry) {
        Some(stake) => stake,
        None => panic_with_error!(env, Error::NoStake),
    }
}

/// La recompensa de `key`, si existe, es de `funder` y sigue abierta.
fn read_bounty(env: &Env, key: &DataKey, funder: &Address) -> Bounty {
    let bounty: Bounty = match env.storage().persistent().get(key) {
        Some(bounty) => bounty,
        None => panic_with_error!(env, Error::NoBounty),
    };
    if bounty.funder != *funder {
        panic_with_error!(env, Error::NoBounty);
    }
    if !bounty.open {
        panic_with_error!(env, Error::BountyClosed);
    }
    bounty
}

fn read_pool(env: &Env, key: &DataKey) -> Pool {
    match env.storage().persistent().get(key) {
        Some(pool) => pool,
        None => panic_with_error!(env, Error::NoPool),
    }
}

/// Lleva al TTL máximo de la red una entrada persistente que no tiene fecha de vencimiento (si
/// se archiva, se restaura con su saldo intacto).
fn keep_max(env: &Env, key: &DataKey) {
    let max = env.storage().max_ttl();
    env.storage().persistent().extend_ttl(key, max, max);
}

fn read_campaign(env: &Env, campaign_id: u32) -> Campaign {
    match env
        .storage()
        .persistent()
        .get(&DataKey::Campaign(campaign_id))
    {
        Some(campaign) => campaign,
        None => panic_with_error!(env, Error::NoCampaign),
    }
}

/// Las entradas con plazo (campañas, recompensas, vales y Pases PaperPay) viven hasta su plazo y
/// `REFUND_GRACE_DAYS` más (y el margen de `keep_until`): así cada aporte de una campaña fallida se
/// puede devolver, una recompensa vencida se puede cancelar y un pase vencido se puede liquidar.
fn keep_campaign(env: &Env, key: &DataKey, deadline: u64) {
    keep_until(env, key, deadline + REFUND_GRACE_DAYS as u64 * DAY_SECONDS);
}

/// Ledger en que vence la autorización de una suscripción que cubre `total_days`: al menos un día
/// después de la última renovación. Se redondea al día de ledgers porque es un argumento de
/// `approve`, que quien lee firma: si dependiera del ledger exacto, el valor simulado y el
/// ejecutado no coincidirían y la red rechazaría la firma. Solo cambia si entre simular y enviar
/// la red cruza un múltiplo de `DAY_IN_LEDGERS` (una vez al día); entonces basta reintentar.
fn allowance_until(env: &Env, total_days: u32) -> u32 {
    (env.ledger().sequence() / DAY_IN_LEDGERS + total_days + 2) * DAY_IN_LEDGERS
}

/// Días entre 1 y `MAX_PASS_DAYS`, y `amount` exactamente `days × day_price`.
fn check_price(env: &Env, days: u32, day_price: i128, amount: i128) {
    if days == 0 || days > MAX_PASS_DAYS {
        panic_with_error!(env, Error::InvalidDays);
    }
    let expected = match day_price.checked_mul(days as i128) {
        Some(total) => total,
        None => panic_with_error!(env, Error::AmountOverflow),
    };
    if amount != expected {
        panic_with_error!(env, Error::WrongPrice);
    }
}

fn read_config(env: &Env) -> Config {
    env.storage().instance().get(&DataKey::Config).unwrap()
}

fn extend_instance_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_EXTEND_TO);
}

fn validate_paper_id(env: &Env, paper_id: &String) {
    if paper_id.is_empty() || paper_id.len() > MAX_PAPER_ID_LEN {
        panic_with_error!(env, Error::InvalidPaperId);
    }
}

/// Partes de `pay_authors`: de 1 a `MAX_AUTHORS` autores distintos, cada parte entre 1 y 10 000
/// puntos base, y entre todas exactamente 10 000 (100%).
fn validate_shares(env: &Env, authors: &Vec<Share>) {
    if authors.len() > MAX_AUTHORS {
        panic_with_error!(env, Error::TooManyAuthors);
    }
    if authors.is_empty() {
        panic_with_error!(env, Error::InvalidShares);
    }
    let mut seen: Map<Address, bool> = Map::new(env);
    let mut total: u32 = 0;
    for share in authors.iter() {
        if share.bps == 0 || share.bps as i128 > BPS_DENOMINATOR {
            panic_with_error!(env, Error::InvalidShares);
        }
        if seen.contains_key(share.account.clone()) {
            panic_with_error!(env, Error::InvalidShares);
        }
        seen.set(share.account, true);
        // Hasta `MAX_AUTHORS` partes de 10 000 como máximo: no se desborda.
        total += share.bps;
    }
    if total as i128 != BPS_DENOMINATOR {
        panic_with_error!(env, Error::InvalidShares);
    }
}

/// Reparte `total` entre `shares` según sus puntos base. Cada parte se redondea hacia abajo y lo
/// que sobra por el redondeo va a la primera (el autor de correspondencia o el primer ganador):
/// las partes suman exactamente `total`. `shares` ya pasó por `validate_shares`.
fn share_parts(env: &Env, total: i128, shares: &Vec<Share>) -> Vec<i128> {
    let mut parts: Vec<i128> = Vec::new(env);
    let mut assigned: i128 = 0;
    for share in shares.iter() {
        let part = match total.checked_mul(share.bps as i128) {
            Some(scaled) => scaled / BPS_DENOMINATOR,
            None => panic_with_error!(env, Error::AmountOverflow),
        };
        // Cada parte es un piso de `total × bps / 10 000`: la suma nunca pasa de `total`.
        assigned += part;
        parts.push_back(part);
    }
    parts.set(0, parts.get_unchecked(0) + (total - assigned));
    parts
}

fn add(env: &Env, a: i128, b: i128) -> i128 {
    match a.checked_add(b) {
        Some(sum) => sum,
        None => panic_with_error!(env, Error::AmountOverflow),
    }
}

/// La comisión se redondea hacia abajo: cualquier fracción de stroop queda para la editorial.
fn compute_split(env: &Env, amount: i128, fee_bps: u32) -> Split {
    if amount <= 0 {
        panic_with_error!(env, Error::InvalidAmount);
    }
    let fee = match amount.checked_mul(fee_bps as i128) {
        Some(scaled) => scaled / BPS_DENOMINATOR,
        None => panic_with_error!(env, Error::AmountOverflow),
    };
    Split {
        publisher: amount - fee,
        fee,
    }
}

#[cfg(test)]
mod test;
